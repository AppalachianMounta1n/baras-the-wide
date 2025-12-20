//! Overlay spawning and lifecycle management
//!
//! Generic spawn function and factory functions for creating overlays.

use std::thread::{self, JoinHandle};
use tokio::sync::mpsc::{self, Sender};

use baras_core::context::{OverlayAppearanceConfig, OverlayPositionConfig, PersonalOverlayConfig};
use baras_overlay::{MetricOverlay, Overlay, OverlayConfig, PersonalOverlay};

use super::state::{OverlayCommand, OverlayHandle, PositionEvent};
use super::types::{OverlayType, MetricType};

// ─────────────────────────────────────────────────────────────────────────────
// Generic Spawn Function
// ─────────────────────────────────────────────────────────────────────────────

/// Generic spawn function for any overlay implementing the Overlay trait
///
/// This unified event loop handles:
/// - Command processing (move mode, data updates, config updates, position queries)
/// - Window event polling
/// - Render scheduling based on interaction state
/// - Resize corner state tracking
pub fn spawn_overlay<O: Overlay>(
    mut overlay: O,
    kind: OverlayType,
) -> (Sender<OverlayCommand>, JoinHandle<()>) {
    let (tx, mut rx) = mpsc::channel::<OverlayCommand>(32);

    let handle = thread::spawn(move || {
        let mut needs_render = true;
        let mut was_in_resize_corner = false;
        let mut was_resizing = false;

        loop {
            // Process all pending commands
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    OverlayCommand::SetMoveMode(enabled) => {
                        overlay.set_click_through(!enabled);
                        needs_render = true;
                    }
                    OverlayCommand::UpdateData(data) => {
                        overlay.update_data(data);
                        needs_render = true;
                    }
                    OverlayCommand::UpdateConfig(config) => {
                        overlay.update_config(config);
                        needs_render = true;
                    }
                    OverlayCommand::GetPosition(response_tx) => {
                        let pos = overlay.position();
                        let current_monitor = overlay.frame().window().current_monitor();
                        let (monitor_id, monitor_x, monitor_y) = current_monitor
                            .map(|m| (Some(m.id), m.x, m.y))
                            .unwrap_or((None, 0, 0));
                        let _ = response_tx.send(PositionEvent {
                            kind,
                            x: pos.x,
                            y: pos.y,
                            width: pos.width,
                            height: pos.height,
                            monitor_id,
                            monitor_x,
                            monitor_y,
                        });
                    }
                    OverlayCommand::Shutdown => {
                        return;
                    }
                }
            }

            // Poll window events (returns false if window should close)
            if !overlay.poll_events() {
                break;
            }

            // Check for pending resize
            if overlay.frame().window().pending_size().is_some() {
                needs_render = true;
            }

            // Clear position dirty flag (position is saved on lock, not continuously)
            let _ = overlay.take_position_dirty();

            // Check if resize corner state changed (need to show/hide grip)
            let in_resize_corner = overlay.in_resize_corner();
            let is_resizing = overlay.is_resizing();
            if in_resize_corner != was_in_resize_corner || is_resizing != was_resizing {
                needs_render = true;
                was_in_resize_corner = in_resize_corner;
                was_resizing = is_resizing;
            }

            let is_interactive = overlay.is_interactive();

            if needs_render {
                overlay.render();
                needs_render = false;
            }

            // Sleep longer when locked (no interaction), shorter when interactive
            let sleep_ms = if is_interactive { 16 } else { 50 };
            thread::sleep(std::time::Duration::from_millis(sleep_ms));
        }
    });

    (tx, handle)
}

// ─────────────────────────────────────────────────────────────────────────────
// Factory Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Create and spawn a metric overlay
///
/// Position is stored as relative to the saved monitor. On Wayland with layer-shell,
/// positions are used directly as margins from the output's top-left corner.
/// The target_monitor_id binds the surface to the correct output.
pub fn create_metric_overlay(
    overlay_type: MetricType,
    position: OverlayPositionConfig,
    appearance: OverlayAppearanceConfig,
    background_alpha: u8,
) -> Result<OverlayHandle, String> {
    // Position is already relative to the monitor - pass directly
    // On Wayland: used as layer-shell margins
    // On Windows: will be converted to absolute using monitor position
    let config = OverlayConfig {
        x: position.x,
        y: position.y,
        width: position.width,
        height: position.height,
        namespace: overlay_type.namespace().to_string(),
        click_through: true,
        target_monitor_id: position.monitor_id.clone(),
    };

    let overlay = MetricOverlay::new(config, overlay_type.title(), appearance, background_alpha)
        .map_err(|e| format!("Failed to create {} overlay: {}", overlay_type.title(), e))?;

    let kind = OverlayType::Metric(overlay_type);
    let (tx, handle) = spawn_overlay(overlay, kind);

    Ok(OverlayHandle { tx, handle, kind })
}

/// Create and spawn the personal overlay
///
/// Position is stored as relative to the saved monitor. On Wayland with layer-shell,
/// positions are used directly as margins from the output's top-left corner.
/// The target_monitor_id binds the surface to the correct output.
pub fn create_personal_overlay(
    position: OverlayPositionConfig,
    personal_config: PersonalOverlayConfig,
    background_alpha: u8,
) -> Result<OverlayHandle, String> {
    // Position is already relative to the monitor - pass directly
    let config = OverlayConfig {
        x: position.x,
        y: position.y,
        width: position.width,
        height: position.height,
        namespace: "baras-personal".to_string(),
        click_through: true,
        target_monitor_id: position.monitor_id.clone(),
    };

    let overlay = PersonalOverlay::new(config, personal_config, background_alpha)
        .map_err(|e| format!("Failed to create personal overlay: {}", e))?;

    let kind = OverlayType::Personal;
    let (tx, handle) = spawn_overlay(overlay, kind);

    Ok(OverlayHandle { tx, handle, kind })
}
