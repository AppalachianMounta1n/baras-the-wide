use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

use baras_core::context::{AppConfig, DirectoryIndex, ParsingSession};

// ─────────────────────────────────────────────────────────────────────────────
// Raid Slot Registry
// ─────────────────────────────────────────────────────────────────────────────

/// Information about a player registered in the raid frame
#[derive(Debug, Clone)]
pub struct RegisteredPlayer {
    pub entity_id: i64,
    pub name: String,
    pub class_id: Option<i64>,
    pub discipline_id: Option<i64>,
}

impl RegisteredPlayer {
    pub fn new(entity_id: i64, name: String) -> Self {
        Self {
            entity_id,
            name,
            class_id: None,
            discipline_id: None,
        }
    }
}

/// Tracks persistent player-to-slot assignments for raid frames.
///
/// Players are added when they receive an effect from the local player.
/// Players stay in their assigned slot until explicitly removed by user action.
#[derive(Debug, Default)]
pub struct RaidSlotRegistry {
    /// Maps slot (0-15) → registered player info
    slots: HashMap<u8, RegisteredPlayer>,
    /// Reverse lookup: entity_id → slot
    entity_to_slot: HashMap<i64, u8>,
    /// Maximum number of slots (configurable, default 8)
    max_slots: u8,
}

impl RaidSlotRegistry {
    pub fn new(max_slots: u8) -> Self {
        Self {
            slots: HashMap::new(),
            entity_to_slot: HashMap::new(),
            max_slots,
        }
    }

    /// Try to register a player in the first available slot.
    /// Returns `Some(slot)` if newly registered, `None` if already registered or full.
    /// This is the primary registration method - duplicates are silently rejected.
    pub fn try_register(&mut self, entity_id: i64, name: String) -> Option<u8> {
        // Already registered - reject
        if self.entity_to_slot.contains_key(&entity_id) {
            return None;
        }

        // Find first available slot (returns None if all full)
        let slot = self.find_first_available_slot()?;
        let player = RegisteredPlayer::new(entity_id, name);

        self.slots.insert(slot, player);
        self.entity_to_slot.insert(entity_id, slot);

        eprintln!("[RAID-REGISTRY] Registered player {} ({}) in slot {}", entity_id, self.slots.get(&slot).unwrap().name, slot);
        Some(slot)
    }

    /// Update player's class/discipline from DisciplineChanged event
    pub fn update_discipline(&mut self, entity_id: i64, class_id: i64, discipline_id: i64) {
        if let Some(&slot) = self.entity_to_slot.get(&entity_id) {
            if let Some(player) = self.slots.get_mut(&slot) {
                player.class_id = Some(class_id);
                player.discipline_id = Some(discipline_id);
            }
        }
    }

    /// Update player's name (if we get better info later)
    pub fn update_name(&mut self, entity_id: i64, name: String) {
        if let Some(&slot) = self.entity_to_slot.get(&entity_id) {
            if let Some(player) = self.slots.get_mut(&slot) {
                player.name = name;
            }
        }
    }

    /// Find the first available slot (lowest numbered empty slot)
    fn find_first_available_slot(&self) -> Option<u8> {
        for slot in 0..self.max_slots {
            if !self.slots.contains_key(&slot) {
                return Some(slot);
            }
        }
        None // All slots full
    }

    /// Swap two slots (user-initiated rearrange)
    pub fn swap_slots(&mut self, slot_a: u8, slot_b: u8) {
        let player_a = self.slots.remove(&slot_a);
        let player_b = self.slots.remove(&slot_b);

        if let Some(p) = player_a {
            self.entity_to_slot.insert(p.entity_id, slot_b);
            self.slots.insert(slot_b, p);
        }
        if let Some(p) = player_b {
            self.entity_to_slot.insert(p.entity_id, slot_a);
            self.slots.insert(slot_a, p);
        }

        eprintln!("[RAID-REGISTRY] Swapped slots {} <-> {}", slot_a, slot_b);
    }

    /// Remove player from a specific slot (user-initiated delete)
    pub fn remove_slot(&mut self, slot: u8) {
        if let Some(player) = self.slots.remove(&slot) {
            self.entity_to_slot.remove(&player.entity_id);
            eprintln!("[RAID-REGISTRY] Removed player {} from slot {}", player.entity_id, slot);
        }
    }

    /// Get the slot for an entity (if registered)
    pub fn get_slot(&self, entity_id: i64) -> Option<u8> {
        self.entity_to_slot.get(&entity_id).copied()
    }

    /// Get the player in a specific slot
    pub fn get_player(&self, slot: u8) -> Option<&RegisteredPlayer> {
        self.slots.get(&slot)
    }

    /// Check if a player is registered
    pub fn is_registered(&self, entity_id: i64) -> bool {
        self.entity_to_slot.contains_key(&entity_id)
    }

    /// Clear all assignments (new session/encounter)
    pub fn clear(&mut self) {
        self.slots.clear();
        self.entity_to_slot.clear();
        eprintln!("[RAID-REGISTRY] Cleared all slots");
    }

    /// Iterate over all registered players with their slots
    pub fn iter(&self) -> impl Iterator<Item = (u8, &RegisteredPlayer)> {
        self.slots.iter().map(|(&slot, player)| (slot, player))
    }

    /// Number of registered players
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Maximum slots configured
    pub fn max_slots(&self) -> u8 {
        self.max_slots
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared State
// ─────────────────────────────────────────────────────────────────────────────

/// State shared between the service and Tauri commands
pub struct SharedState {
    pub config: RwLock<AppConfig>,
    pub directory_index: RwLock<DirectoryIndex>,
    pub session: RwLock<Option<Arc<RwLock<ParsingSession>>>>,
    /// Whether we're currently in active combat (for metrics updates)
    pub in_combat: AtomicBool,
    /// Whether the directory watcher is active
    pub watching: AtomicBool,
    /// Raid frame slot assignments (persists player positions)
    pub raid_registry: Mutex<RaidSlotRegistry>,
}

impl SharedState {
    pub fn new(config: AppConfig, directory_index: DirectoryIndex) -> Self {
        Self {
            config: RwLock::new(config),
            directory_index: RwLock::new(directory_index),
            session: RwLock::new(None),
            in_combat: AtomicBool::new(false),
            watching: AtomicBool::new(false),
            raid_registry: Mutex::new(RaidSlotRegistry::new(8)), // Default 8 slots (2x4 grid)
        }
    }

    pub async fn with_session<F, T>(&self, f: F) -> Option<T>
    where
        F: FnOnce(&mut ParsingSession) -> T,
    {
        let session_lock = self.session.read().await;
        if let Some(session_arc) = &*session_lock {
            let mut session = session_arc.write().await;
            Some(f(&mut session))
        } else {
            None
        }
    }


}
