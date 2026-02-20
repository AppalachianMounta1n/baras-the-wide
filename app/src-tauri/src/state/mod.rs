//! Application state management
//!
//! This module contains all shared state types used across the Tauri application:
//! - `SharedState`: Core application state shared between service and commands
//! - `RaidSlotRegistry`: Persistent player-to-slot assignments for raid frames

mod raid_registry;

pub use raid_registry::{RaidSlotRegistry, RegisteredPlayer};

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

use baras_core::context::{AppConfig, DirectoryIndex, LogAreaCache, ParsingSession};
use baras_core::query::QueryContext;

/// State shared between the combat service and Tauri commands.
///
/// This is the central state container that coordinates:
/// - Configuration (persisted to disk)
/// - Directory index (log files available)
/// - Current parsing session (if tailing)
/// - Combat state flags
/// - Raid frame slot assignments
pub struct SharedState {
    /// Application configuration (persisted to disk)
    pub config: RwLock<AppConfig>,
    /// Index of log files in the configured directory
    pub directory_index: RwLock<DirectoryIndex>,
    /// Current parsing session (when tailing a log file)
    pub session: RwLock<Option<Arc<RwLock<ParsingSession>>>>,
    /// Whether we're currently in active combat (for metrics updates)
    pub in_combat: AtomicBool,
    /// Whether the directory watcher is active
    pub watching: AtomicBool,
    /// Whether we're in live tailing mode (vs viewing historical file)
    pub is_live_tailing: AtomicBool,
    /// Raid frame slot assignments (persists player positions)
    pub raid_registry: Mutex<RaidSlotRegistry>,
    /// Current area ID for lazy loading timers (0 = unknown)
    pub current_area_id: AtomicI64,

    // ─── Overlay status flags (for skipping work when not needed) ───
    /// Whether raid overlay is currently running
    pub raid_overlay_active: AtomicBool,
    /// Whether boss health overlay is currently running
    pub boss_health_overlay_active: AtomicBool,
    /// Whether timer overlay is currently running
    pub timer_overlay_active: AtomicBool,
    /// Whether effects A overlay is currently running
    pub effects_a_overlay_active: AtomicBool,
    /// Whether effects B overlay is currently running
    pub effects_b_overlay_active: AtomicBool,
    /// Whether cooldowns overlay is currently running
    pub cooldowns_overlay_active: AtomicBool,
    /// Whether DOT tracker overlay is currently running
    pub dot_tracker_overlay_active: AtomicBool,
    /// Whether raid frame rearrange mode is active (bypasses rendering gates)
    pub rearrange_mode: AtomicBool,

    // ─── Conversation auto-hide state ───────────────────────────────────────
    /// Whether overlays are temporarily hidden due to conversation
    pub conversation_hiding_active: AtomicBool,
    /// Whether overlays were visible before conversation started (for restore)
    pub overlays_visible_before_conversation: AtomicBool,

    // ─── Not-live auto-hide state ────────────────────────────────────────────
    /// Whether overlays are temporarily hidden because session is not live
    pub not_live_hiding_active: AtomicBool,
    /// Whether overlays were visible before not-live hide triggered (for restore)
    pub overlays_visible_before_not_live: AtomicBool,

    /// Shared query context for DataFusion queries (reuses SessionContext)
    pub query_context: QueryContext,

    /// Cache of area indexes for log files (persisted to disk)
    pub area_cache: RwLock<LogAreaCache>,
}

impl SharedState {
    pub fn new(config: AppConfig, directory_index: DirectoryIndex) -> Self {
        let raid_slots = config.overlay_settings.raid_overlay.total_slots();
        Self {
            config: RwLock::new(config),
            directory_index: RwLock::new(directory_index),
            session: RwLock::new(None),
            in_combat: AtomicBool::new(false),
            watching: AtomicBool::new(false),
            is_live_tailing: AtomicBool::new(true), // Start in live tailing mode
            raid_registry: Mutex::new(RaidSlotRegistry::new(raid_slots)),
            current_area_id: AtomicI64::new(0),
            // Overlay status flags - updated by OverlayManager
            raid_overlay_active: AtomicBool::new(false),
            boss_health_overlay_active: AtomicBool::new(false),
            timer_overlay_active: AtomicBool::new(false),
            effects_a_overlay_active: AtomicBool::new(false),
            effects_b_overlay_active: AtomicBool::new(false),
            cooldowns_overlay_active: AtomicBool::new(false),
            dot_tracker_overlay_active: AtomicBool::new(false),
            rearrange_mode: AtomicBool::new(false),
            // Conversation auto-hide state
            conversation_hiding_active: AtomicBool::new(false),
            overlays_visible_before_conversation: AtomicBool::new(false),
            // Not-live auto-hide state
            not_live_hiding_active: AtomicBool::new(false),
            overlays_visible_before_not_live: AtomicBool::new(false),
            // Shared query context for DataFusion (reuses SessionContext across queries)
            query_context: QueryContext::new(),
            // Area cache - loaded from disk later in service startup
            area_cache: RwLock::new(LogAreaCache::new()),
        }
    }

    /// Check if the current session is "not live" — i.e. historical, stale, or has no player.
    /// Returns `true` if overlays should be auto-hidden.
    pub async fn is_session_not_live(&self) -> bool {
        if !self.is_live_tailing.load(Ordering::SeqCst) {
            return true;
        }

        let session_guard = self.session.read().await;
        let Some(session) = session_guard.as_ref() else {
            return true;
        };

        let s = session.read().await;
        let has_player = s
            .session_cache
            .as_ref()
            .map(|c| c.player_initialized)
            .unwrap_or(false);

        if !has_player {
            return true;
        }

        // Stale check: no events in the last 15 minutes
        let last_activity = s.last_event_time.or(s.game_session_date);
        if let Some(last) = last_activity {
            let elapsed = chrono::Local::now().naive_local().signed_duration_since(last);
            if elapsed > chrono::Duration::minutes(15) {
                return true;
            }
        }

        false
    }

    /// Mark overlays as auto-hidden due to not-live state.
    /// Stores that overlays were visible before hiding so they can be restored.
    pub fn activate_not_live_hiding(&self) {
        self.not_live_hiding_active.store(true, Ordering::SeqCst);
        self.overlays_visible_before_not_live
            .store(true, Ordering::SeqCst);
    }

    /// Clear the not-live auto-hide state and return whether overlays should be restored.
    pub fn deactivate_not_live_hiding(&self) -> bool {
        if !self.not_live_hiding_active.load(Ordering::SeqCst) {
            return false;
        }
        self.not_live_hiding_active.store(false, Ordering::SeqCst);
        let was_visible = self.overlays_visible_before_not_live.load(Ordering::SeqCst);
        self.overlays_visible_before_not_live
            .store(false, Ordering::SeqCst);
        was_visible
    }

    /// Execute a function with mutable access to the current session.
    /// Returns `None` if no session is active.
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
