//! Tauri commands for overlay management
//!
//! All Tauri-invokable commands for showing, hiding, and configuring overlays.

use baras_core::context::OverlayPositionConfig;
use baras_overlay::{OverlayConfigUpdate, OverlayData};
use serde::Serialize;
use tauri::State;

use baras_overlay::{RaidGridLayout, RaidOverlayConfig};

use super::metrics::create_entries_for_type;
use super::spawn::{create_metric_overlay, create_personal_overlay, create_raid_overlay};
use super::state::OverlayCommand;
use super::types::{OverlayType, MetricType};
use super::SharedOverlayState;

// ─────────────────────────────────────────────────────────────────────────────
// Response Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct OverlayStatusResponse {
    pub running: Vec<MetricType>,
    pub enabled: Vec<MetricType>,
    pub personal_running: bool,
    pub personal_enabled: bool,
    pub raid_running: bool,
    pub raid_enabled: bool,
    pub overlays_visible: bool,
    pub move_mode: bool,
    pub rearrange_mode: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Overlay Show/Hide Commands
// ─────────────────────────────────────────────────────────────────────────────

/// Enable an overlay (persists to config, only shows if overlays_visible is true)
#[tauri::command]
pub async fn show_overlay(
    kind: OverlayType,
    state: State<'_, SharedOverlayState>,
    service: State<'_, crate::service::ServiceHandle>,
) -> Result<bool, String> {
    // Get current config and update enabled state
    let mut config = service.config().await;
    config.overlay_settings.set_enabled(kind.config_key(), true);

    // Save config immediately
    service.update_config(config.clone()).await?;

    // Only spawn overlay if global visibility is enabled
    if !config.overlay_settings.overlays_visible {
        return Ok(true);
    }

    // Check if already running
    {
        let state = state.lock().map_err(|e| e.to_string())?;
        if state.is_running(kind) {
            return Ok(true);
        }
    }

    // Load position from config
    let position = config.overlay_settings.get_position(kind.config_key());
    let needs_monitor_id_save = position.monitor_id.is_none();

    // Create and spawn overlay based on kind (with per-category opacity)
    let overlay_handle = match kind {
        OverlayType::Metric(overlay_type) => {
            let appearance = super::get_appearance_for_type(&config.overlay_settings, overlay_type);
            create_metric_overlay(overlay_type, position, appearance, config.overlay_settings.metric_opacity)?
        }
        OverlayType::Personal => {
            let personal_config = config.overlay_settings.personal_overlay.clone();
            create_personal_overlay(position, personal_config, config.overlay_settings.personal_opacity)?
        }
        OverlayType::Raid => {
            // Load layout and config from saved settings
            let raid_settings = &config.overlay_settings.raid_overlay;
            let layout = RaidGridLayout::from_config(raid_settings);
            let raid_config: RaidOverlayConfig = raid_settings.clone().into();
            create_raid_overlay(position, layout, raid_config, config.overlay_settings.raid_opacity)?
        }
    };
    let tx = overlay_handle.tx.clone();

    // Update state
    {
        let mut state = state.lock().map_err(|e| e.to_string())?;
        state.insert(overlay_handle);
    }

    // Sync move mode state - if app is in move mode, new overlay should be too
    let current_move_mode = {
        state.lock().map_err(|e| e.to_string())?.move_mode
    };
    if current_move_mode {
        let _ = tx.send(OverlayCommand::SetMoveMode(true)).await;
    }

    // Send current data if tailing
    if service.is_tailing().await
        && let Some(data) = service.current_combat_data().await
        && !data.metrics.is_empty()
    {
        match kind {
            OverlayType::Metric(overlay_type) => {
                let entries = create_entries_for_type(overlay_type, &data.metrics);
                let _ = tx.send(OverlayCommand::UpdateData(OverlayData::Metrics(entries))).await;
            }
            OverlayType::Personal => {
                if let Some(stats) = data.to_personal_stats() {
                    let _ = tx.send(OverlayCommand::UpdateData(OverlayData::Personal(stats))).await;
                }
            }
            OverlayType::Raid => {
                // Raid overlay gets data via EffectsUpdated channel, starts empty
            }
        }
    }

    // If monitor_id was None, query and save the position to persist the monitor
    // the compositor chose. This ensures next spawn goes to same monitor.
    if needs_monitor_id_save {
        // Give overlay a moment to be placed by compositor
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let (pos_tx, pos_rx) = tokio::sync::oneshot::channel();
        let _ = tx.send(OverlayCommand::GetPosition(pos_tx)).await;
        if let Ok(pos) = pos_rx.await {
            let relative_x = pos.x - pos.monitor_x;
            let relative_y = pos.y - pos.monitor_y;
            let mut config = service.config().await;
            config.overlay_settings.set_position(
                kind.config_key(),
                OverlayPositionConfig {
                    x: relative_x,
                    y: relative_y,
                    width: pos.width,
                    height: pos.height,
                    monitor_id: pos.monitor_id,
                },
            );
            let _ = service.update_config(config).await;
        }
    }

    Ok(true)
}

/// Disable an overlay (persists to config, hides if currently running)
#[tauri::command]
pub async fn hide_overlay(
    kind: OverlayType,
    state: State<'_, SharedOverlayState>,
    service: State<'_, crate::service::ServiceHandle>,
) -> Result<bool, String> {
    // Get current config and update enabled state
    let mut config = service.config().await;
    config.overlay_settings.set_enabled(kind.config_key(), false);

    // Save config immediately
    service.update_config(config).await?;

    // Shutdown overlay if running
    let overlay_handle = {
        let mut state = state.lock().map_err(|e| e.to_string())?;
        // If hiding raid overlay, clear rearrange mode
        if matches!(kind, OverlayType::Raid) {
            state.rearrange_mode = false;
        }
        state.remove(kind)
    };

    if let Some(handle) = overlay_handle {
        let _ = handle.tx.send(OverlayCommand::Shutdown).await;
        let _ = handle.handle.join();
    }

    Ok(true)
}

// ─────────────────────────────────────────────────────────────────────────────
// Bulk Overlay Commands
// ─────────────────────────────────────────────────────────────────────────────

/// Hide all running overlays and set overlays_visible=false
#[tauri::command]
pub async fn hide_all_overlays(
    state: State<'_, SharedOverlayState>,
    service: State<'_, crate::service::ServiceHandle>,
) -> Result<bool, String> {
    // Update and persist overlays_visible = false
    let mut config = service.config().await;
    config.overlay_settings.overlays_visible = false;
    service.update_config(config).await?;

    // Shutdown all running overlays (both metric and personal are in unified state)
    let handles = {
        let mut state = state.lock().map_err(|e| e.to_string())?;
        state.move_mode = false;
        state.drain()
    };

    for handle in handles {
        let _ = handle.tx.send(OverlayCommand::Shutdown).await;
        let _ = handle.handle.join();
    }

    Ok(true)
}

/// Show all enabled overlays and set overlays_visible=true
#[tauri::command]
pub async fn show_all_overlays(
    state: State<'_, SharedOverlayState>,
    service: State<'_, crate::service::ServiceHandle>,
) -> Result<Vec<MetricType>, String> {
    // Update and persist overlays_visible = true
    let mut config = service.config().await;
    config.overlay_settings.overlays_visible = true;
    service.update_config(config.clone()).await?;

    let enabled_keys = config.overlay_settings.enabled_types();
    let metric_opacity = config.overlay_settings.metric_opacity;
    let personal_opacity = config.overlay_settings.personal_opacity;

    // Get current combat data once for all overlays
    let combat_data = if service.is_tailing().await {
        service.current_combat_data().await
    } else {
        None
    };

    let mut shown_metric_types = Vec::new();
    // Track overlays that need their monitor_id saved: (config_key, tx)
    let mut needs_monitor_save: Vec<(String, tokio::sync::mpsc::Sender<OverlayCommand>)> = Vec::new();

    for key in &enabled_keys {
        if key == "personal" {
            // Handle personal overlay
            let kind = OverlayType::Personal;
            let already_running = {
                let state = state.lock().map_err(|e| e.to_string())?;
                state.is_running(kind)
            };

            if !already_running {
                let position = config.overlay_settings.get_position("personal");
                let needs_save = position.monitor_id.is_none();
                let personal_config = config.overlay_settings.personal_overlay.clone();
                let overlay_handle = create_personal_overlay(position, personal_config, personal_opacity)?;
                let tx = overlay_handle.tx.clone();

                {
                    let mut state = state.lock().map_err(|e| e.to_string())?;
                    state.insert(overlay_handle);
                }

                // Send initial personal stats if available
                if let Some(ref data) = combat_data
                    && let Some(stats) = data.to_personal_stats()
                {
                    let _ = tx.send(OverlayCommand::UpdateData(OverlayData::Personal(stats))).await;
                }

                if needs_save {
                    needs_monitor_save.push(("personal".to_string(), tx));
                }
            }
        } else if key == "raid" {
            // Handle raid overlay
            let kind = OverlayType::Raid;
            let already_running = {
                let state = state.lock().map_err(|e| e.to_string())?;
                state.is_running(kind)
            };

            if !already_running {
                let position = config.overlay_settings.get_position("raid");
                let needs_save = position.monitor_id.is_none();
                let raid_settings = &config.overlay_settings.raid_overlay;
                let layout = RaidGridLayout::from_config(raid_settings);
                let raid_config: RaidOverlayConfig = raid_settings.clone().into();
                let overlay_handle = create_raid_overlay(position, layout, raid_config, config.overlay_settings.raid_opacity)?;
                let tx = overlay_handle.tx.clone();

                {
                    let mut state = state.lock().map_err(|e| e.to_string())?;
                    state.insert(overlay_handle);
                }

                // Raid overlay gets data via EffectsUpdated channel, starts empty

                if needs_save {
                    needs_monitor_save.push(("raid".to_string(), tx));
                }
            }
        } else if let Some(overlay_type) = MetricType::from_config_key(key) {
            // Handle metric overlay
            let kind = OverlayType::Metric(overlay_type);

            // Check if already running
            {
                let state = state.lock().map_err(|e| e.to_string())?;
                if state.is_running(kind) {
                    shown_metric_types.push(overlay_type);
                    continue;
                }
            }

            // Load position, appearance, and spawn
            let position = config.overlay_settings.get_position(key);
            let needs_save = position.monitor_id.is_none();
            let appearance = super::get_appearance_for_type(&config.overlay_settings, overlay_type);
            let overlay_handle = create_metric_overlay(overlay_type, position, appearance, metric_opacity)?;
            let tx = overlay_handle.tx.clone();

            // Update state
            {
                let mut state = state.lock().map_err(|e| e.to_string())?;
                state.insert(overlay_handle);
            }

            // Send current metrics if available
            if let Some(ref data) = combat_data
                && !data.metrics.is_empty()
            {
                let entries = create_entries_for_type(overlay_type, &data.metrics);
                let _ = tx.send(OverlayCommand::UpdateData(OverlayData::Metrics(entries))).await;
            }

            if needs_save {
                needs_monitor_save.push((key.clone(), tx.clone()));
            }

            shown_metric_types.push(overlay_type);
        }
    }

    // Save monitor_id for overlays that didn't have one
    if !needs_monitor_save.is_empty() {
        // Give overlays a moment to be placed by compositor
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let mut config = service.config().await;
        for (key, tx) in needs_monitor_save {
            let (pos_tx, pos_rx) = tokio::sync::oneshot::channel();
            let _ = tx.send(OverlayCommand::GetPosition(pos_tx)).await;
            if let Ok(pos) = pos_rx.await {
                let relative_x = pos.x - pos.monitor_x;
                let relative_y = pos.y - pos.monitor_y;
                config.overlay_settings.set_position(
                    &key,
                    OverlayPositionConfig {
                        x: relative_x,
                        y: relative_y,
                        width: pos.width,
                        height: pos.height,
                        monitor_id: pos.monitor_id,
                    },
                );
            }
        }
        let _ = service.update_config(config).await;
    }

    Ok(shown_metric_types)
}

// ─────────────────────────────────────────────────────────────────────────────
// Move Mode and Status
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn toggle_move_mode(
    state: State<'_, SharedOverlayState>,
    service: State<'_, crate::service::ServiceHandle>,
) -> Result<bool, String> {
    let (txs, new_mode, raid_tx, was_rearranging) = {
        let mut state = state.lock().map_err(|e| e.to_string())?;
        if !state.any_running() {
            return Err("No overlays running".to_string());
        }
        state.move_mode = !state.move_mode;
        let was_rearranging = state.rearrange_mode;
        // Move mode overrides rearrange mode
        if state.move_mode {
            state.rearrange_mode = false;
        }
        let txs: Vec<_> = state.all_txs().into_iter().cloned().collect();
        let raid_tx = state.get_raid_tx().cloned();
        (txs, state.move_mode, raid_tx, was_rearranging)
    };

    // If we were in rearrange mode, turn it off first
    if was_rearranging && new_mode
        && let Some(tx) = &raid_tx {
            let _ = tx.send(OverlayCommand::SetRearrangeMode(false)).await;
    }

    // Send to all overlays
    for tx in &txs {
        let _ = tx.send(OverlayCommand::SetMoveMode(new_mode)).await;
    }

    // When locking (move_mode = false), save all overlay positions
    if !new_mode {
        let mut positions = Vec::new();
        for tx in &txs {
            let (pos_tx, pos_rx) = tokio::sync::oneshot::channel();
            let _ = tx.send(OverlayCommand::GetPosition(pos_tx)).await;
            if let Ok(pos) = pos_rx.await {
                positions.push(pos);
            }
        }

        // Save positions to config (relative to monitor)
        let mut config = service.config().await;
        for pos in positions {
            // Convert absolute screen position to relative monitor position
            let relative_x = pos.x - pos.monitor_x;
            let relative_y = pos.y - pos.monitor_y;

            config.overlay_settings.set_position(
                pos.kind.config_key(),
                OverlayPositionConfig {
                    x: relative_x,
                    y: relative_y,
                    width: pos.width,
                    height: pos.height,
                    monitor_id: pos.monitor_id.clone(),
                },
            );
        }
        service.update_config(config).await.map_err(|e| e.to_string())?;
    }

    Ok(new_mode)
}

#[tauri::command]
pub async fn toggle_raid_rearrange(
    state: State<'_, SharedOverlayState>,
) -> Result<bool, String> {
    let (raid_tx, new_mode) = {
        let mut state = state.lock().map_err(|e| e.to_string())?;
        if !state.is_raid_running() {
            return Err("Raid overlay not running".to_string());
        }
        state.rearrange_mode = !state.rearrange_mode;
        let tx = state.get_raid_tx().cloned();
        (tx, state.rearrange_mode)
    };

    // Send to raid overlay only
    if let Some(tx) = raid_tx {
        let _ = tx.send(OverlayCommand::SetRearrangeMode(new_mode)).await;
    }

    Ok(new_mode)
}

#[tauri::command]
pub async fn get_overlay_status(
    state: State<'_, SharedOverlayState>,
    service: State<'_, crate::service::ServiceHandle>,
) -> Result<OverlayStatusResponse, String> {
    let (running_metric_types, personal_running, raid_running, move_mode, rearrange_mode) = {
        let state = state.lock().map_err(|e| e.to_string())?;
        (
            state.running_metric_types(),
            state.is_personal_running(),
            state.is_raid_running(),
            state.move_mode,
            state.rearrange_mode,
        )
    };

    // Get enabled types and visibility from config
    let config = service.config().await;
    let enabled: Vec<MetricType> = config
        .overlay_settings
        .enabled_types()
        .iter()
        .filter_map(|key| MetricType::from_config_key(key))
        .collect();

    let personal_enabled = config.overlay_settings.is_enabled("personal");
    let raid_enabled = config.overlay_settings.is_enabled("raid");

    Ok(OverlayStatusResponse {
        running: running_metric_types,
        enabled,
        personal_running,
        personal_enabled,
        raid_running,
        raid_enabled,
        overlays_visible: config.overlay_settings.overlays_visible,
        move_mode,
        rearrange_mode,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Settings Refresh
// ─────────────────────────────────────────────────────────────────────────────

/// Refresh overlay settings for all running overlays
#[tauri::command]
pub async fn refresh_overlay_settings(
    state: State<'_, SharedOverlayState>,
    service: State<'_, crate::service::ServiceHandle>,
) -> Result<bool, String> {
    let config = service.config().await;
    let metric_opacity = config.overlay_settings.metric_opacity;
    let personal_opacity = config.overlay_settings.personal_opacity;

    // Get all running overlays with their kinds
    let overlays: Vec<_> = {
        let state = state.lock().map_err(|e| e.to_string())?;
        state.all_overlays().into_iter().map(|(k, tx)| (k, tx.clone())).collect()
    };

    // Send updated config to each overlay based on its type (with per-category opacity)
    for (kind, tx) in overlays {
        let config_update = match kind {
            OverlayType::Metric(overlay_type) => {
                let appearance = super::get_appearance_for_type(&config.overlay_settings, overlay_type);
                eprintln!("[REFRESH] Updating {} overlay with appearance: bar_color={:?}, font_color={:?}, max_entries={}, show_header={}, show_footer={}, bg_alpha={}",
                    overlay_type.config_key(), appearance.bar_color, appearance.font_color, appearance.max_entries,
                    appearance.show_header, appearance.show_footer, metric_opacity);
                OverlayConfigUpdate::Metric(appearance, metric_opacity)
            }
            OverlayType::Personal => {
                let personal_config = config.overlay_settings.personal_overlay.clone();
                OverlayConfigUpdate::Personal(personal_config, personal_opacity)
            }
            OverlayType::Raid => {
                // Load raid config from settings
                let raid_settings = &config.overlay_settings.raid_overlay;
                let raid_config: RaidOverlayConfig = raid_settings.clone().into();
                eprintln!("[REFRESH] Updating raid overlay: max_effects={}, effect_size={:.0}, show_role_icons={}, opacity={}",
                    raid_config.max_effects_per_frame, raid_config.effect_size,
                    raid_config.show_role_icons, config.overlay_settings.raid_opacity);
                OverlayConfigUpdate::Raid(raid_config, config.overlay_settings.raid_opacity)
            }
        };
        let _ = tx.send(OverlayCommand::UpdateConfig(config_update)).await;
    }

    Ok(true)
}

// ─────────────────────────────────────────────────────────────────────────────
// Raid Registry Commands
// ─────────────────────────────────────────────────────────────────────────────

/// Clear all players from the raid frame registry
#[tauri::command]
pub async fn clear_raid_registry(
    service: State<'_, crate::service::ServiceHandle>,
) -> Result<(), String> {
    let mut registry = service.shared.raid_registry.lock().map_err(|e| e.to_string())?;
    registry.clear();
    Ok(())
}

/// Swap two slots in the raid frame registry
#[tauri::command]
pub async fn swap_raid_slots(
    slot_a: u8,
    slot_b: u8,
    service: State<'_, crate::service::ServiceHandle>,
) -> Result<(), String> {
    let mut registry = service.shared.raid_registry.lock().map_err(|e| e.to_string())?;
    registry.swap_slots(slot_a, slot_b);
    Ok(())
}

/// Remove a player from a specific slot
#[tauri::command]
pub async fn remove_raid_slot(
    slot: u8,
    service: State<'_, crate::service::ServiceHandle>,
) -> Result<(), String> {
    let mut registry = service.shared.raid_registry.lock().map_err(|e| e.to_string())?;
    registry.remove_slot(slot);
    Ok(())
}
