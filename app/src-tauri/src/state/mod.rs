//! Application state management
//!
//! This module contains all shared state types used across the Tauri application:
//! - `SharedState`: Core application state shared between service and commands
//! - `RaidSlotRegistry`: Persistent player-to-slot assignments for raid frames

mod raid_registry;

pub use raid_registry::{RaidSlotRegistry, RegisteredPlayer};

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

use baras_core::context::{AppConfig, DirectoryIndex, ParsingSession};

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
}

impl SharedState {
    pub fn new(config: AppConfig, directory_index: DirectoryIndex) -> Self {
        Self {
            config: RwLock::new(config),
            directory_index: RwLock::new(directory_index),
            session: RwLock::new(None),
            in_combat: AtomicBool::new(false),
            watching: AtomicBool::new(false),
            is_live_tailing: AtomicBool::new(true), // Start in live tailing mode
            raid_registry: Mutex::new(RaidSlotRegistry::new(8)), // Default 8 slots (2x4 grid)
        }
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
