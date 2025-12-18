use crate::context::resolve;
use crate::session::encounter::{Encounter, EncounterState};
use crate::session::player::{AreaInfo, PlayerInfo};
use std::collections::VecDeque;

const CACHE_DEFAULT_CAPACITY: usize = 3;

/// Pure storage for session state.
/// Routing logic lives in EventProcessor.
#[derive(Debug, Clone, Default)]
pub struct SessionCache {
    // Player state
    pub player: PlayerInfo,
    pub player_initialized: bool,

    // Area state
    pub current_area: AreaInfo,

    // Encounter tracking - fixed-size window
    encounters: VecDeque<Encounter>,
    next_encounter_id: u64,
}

impl SessionCache {
    pub fn new() -> Self {
        let mut cache = Self {
            player: PlayerInfo::default(),
            player_initialized: false,
            current_area: AreaInfo::default(),
            encounters: VecDeque::with_capacity(CACHE_DEFAULT_CAPACITY),
            next_encounter_id: 0,
        };
        cache.push_new_encounter();
        cache
    }

    // --- Encounter Management ---

    pub fn push_new_encounter(&mut self) -> u64 {
        let id = self.next_encounter_id;

        let encounter = if self.player_initialized {
            Encounter::with_player(id, self.player.clone())
        } else {
            Encounter::new(id)
        };

        self.next_encounter_id += 1;
        self.encounters.push_back(encounter);
        self.trim_old_encounters();
        id
    }

    fn trim_old_encounters(&mut self) {
        while self.encounters.len() > CACHE_DEFAULT_CAPACITY {
            self.encounters.pop_front();
        }
    }

    // --- Accessors ---

    pub fn current_encounter(&self) -> Option<&Encounter> {
        self.encounters.back()
    }

    pub fn current_encounter_mut(&mut self) -> Option<&mut Encounter> {
        self.encounters.back_mut()
    }

    pub fn encounters(&self) -> impl Iterator<Item = &Encounter> {
        self.encounters.iter()
    }

    pub fn encounter_by_id(&self, id: u64) -> Option<&Encounter> {
        self.encounters.iter().find(|e| e.id == id)
    }

    pub fn last_combat_encounter(&self) -> Option<&Encounter> {
        self.encounters
            .iter()
            .rfind(|e| e.state != EncounterState::NotStarted)
    }

    pub fn encounter_count(&self) -> usize {
        self.encounters.len()
    }

    // --- Debug/Display ---

    /// Print session and encounter metadata (excludes event lists)
    pub fn print_metadata(&self) {
        let name = resolve(self.player.name);
        println!("=== Session Metadata ===");

        println!("--- Player Info ---");
        println!("  Name: {}", name);
        println!("  ID: {}", self.player.id);
        println!(
            "  Class: {} (id: {})",
            self.player.class_name, self.player.class_id
        );
        println!(
            "  Discipline: {} (id: {})",
            self.player.discipline_name, self.player.discipline_id
        );
        println!("  Initialized: {}", self.player_initialized);
        println!();

        println!("--- Current Area ---");
        println!("  Name: {}", self.current_area.area_name);
        println!("  ID: {}", self.current_area.area_id);
        println!(
            "  Entered at: {}",
            self.current_area
                .entered_at
                .map(|t| t.to_string())
                .unwrap_or_else(|| "N/A".to_string())
        );
        println!();

        println!("--- Encounters ({} cached) ---", self.encounters.len());
        for enc in &self.encounters {
            println!("  Encounter #{}", enc.id);
            println!("    State: {:?}", enc.state);
            println!(
                "    Enter combat: {}",
                enc.enter_combat_time
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "N/A".to_string())
            );
            println!(
                "    Exit combat: {}",
                enc.exit_combat_time
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "N/A".to_string())
            );
            println!(
                "    Duration: {}",
                enc.duration_ms()
                    .map(|ms| format!("{}ms", ms))
                    .unwrap_or_else(|| "N/A".to_string())
            );
            println!("    All Players Dead: {}", enc.all_players_dead);
            println!("    Event count: {}", enc.events.len());
            println!("    Players ({}):", enc.players.len());
            for (id, player) in &enc.players {
                println!(
                    "      [{}: {}] alive={}, death_time={}",
                    resolve(player.name),
                    id,
                    !player.is_dead,
                    player
                        .death_time
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "N/A".to_string())
                );
            }
            println!("    Npcs ({}):", enc.npcs.len());
            for (id, npc) in &enc.npcs {
                println!(
                    "      [{}: {}] alive={}, death_time={}",
                    resolve(npc.name),
                    id,
                    !npc.is_dead,
                    npc.death_time
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "N/A".to_string())
                );
            }
        }
    }
}
