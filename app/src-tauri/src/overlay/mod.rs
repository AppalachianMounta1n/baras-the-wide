//! Overlay management module
//!
//! Handles overlay lifecycle, state management, and Tauri commands.
//!
//! # Module Structure
//!
//! - `types` - Core type definitions (`MetricType`, `OverlayType`)
//! - `state` - Runtime state management (`OverlayState`, `OverlayCommand`, `OverlayHandle`)
//! - `spawn` - Overlay creation and spawning functions
//! - `commands` - Tauri command handlers
//! - `metrics` - Metric entry creation helpers

pub mod commands;
mod metrics;
mod spawn;
mod state;
mod types;

use std::sync::{Arc, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// Shared State Type
// ─────────────────────────────────────────────────────────────────────────────

/// Type alias for shared overlay state (used in Tauri managed state)
pub type SharedOverlayState = Arc<Mutex<state::OverlayState>>;

// ─────────────────────────────────────────────────────────────────────────────
// Re-exports
// ─────────────────────────────────────────────────────────────────────────────

// Types
pub use types::{OverlayType, MetricType};

// State management
pub use state::{OverlayCommand, OverlayHandle, OverlayState, PositionEvent};

// Spawn functions
pub use spawn::{create_metric_overlay, create_personal_overlay, create_raid_overlay};

// Metrics helpers
pub use metrics::{create_all_entries, create_entries_for_type};

// Tauri commands
pub use commands::{
    clear_raid_registry, get_overlay_status, hide_all_overlays, hide_overlay,
    refresh_overlay_settings, remove_raid_slot, show_all_overlays, show_overlay,
    swap_raid_slots, toggle_move_mode, toggle_raid_rearrange, OverlayStatusResponse,
};

// ─────────────────────────────────────────────────────────────────────────────
// Appearance Helper
// ─────────────────────────────────────────────────────────────────────────────

use baras_core::context::{OverlayAppearanceConfig, OverlaySettings};

/// Get appearance for a metric overlay type with correct type-specific defaults.
///
/// If the user has saved custom appearance settings, those are returned.
/// Otherwise, returns the default appearance with the correct bar color for this type.
pub fn get_appearance_for_type(settings: &OverlaySettings, overlay_type: MetricType) -> OverlayAppearanceConfig {
    let key = overlay_type.config_key();
    if let Some(saved) = settings.appearances.get(key) {
        saved.clone()
    } else {
        // No saved appearance - use type-specific default
        overlay_type.default_appearance()
    }
}
