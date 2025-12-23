//! Tauri commands for overlay management
//!
//! All Tauri-invokable commands for showing, hiding, and configuring overlays.

use baras_core::context::OverlayPositionConfig;
use baras_overlay::{OverlayConfigUpdate, OverlayData};
use serde::Serialize;
use tauri::State;

use baras_overlay::{RaidGridLayout, RaidOverlayConfig};

use super::metrics::create_entries_for_type;
use super::spawn::{create_boss_health_overlay, create_metric_overlay, create_personal_overlay, create_raid_overlay};
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
    pub boss_health_running: bool,
    pub boss_health_enabled: bool,
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
        OverlayType::BossHealth => {
            let boss_config = config.overlay_settings.boss_health.clone();
            create_boss_health_overlay(position, boss_config, config.overlay_settings.boss_health_opacity)?
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
            OverlayType::Raid | OverlayType::BossHealth => {
                // Raid and boss health overlays get data via separate update channels
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

/// Implementation of hide_all_overlays (for use by hotkeys and commands)
pub async fn hide_all_overlays_impl(
    state: SharedOverlayState,
    service: crate::service::ServiceHandle,
) -> Result<bool, String> {
    // Update and persist overlays_visible = false
    let mut config = service.config().await;
    config.overlay_settings.overlays_visible = false;
    service.update_config(config).await?;

    // Shutdown all running overlays
    let handles = {
        let mut state = state.lock().map_err(|e| e.to_string())?;
        state.move_mode = false;
        state.overlays_visible = false;
        state.drain()
    };

    for handle in handles {
        let _ = handle.tx.send(OverlayCommand::Shutdown).await;
        let _ = handle.handle.join();
    }

    Ok(true)
}

/// Implementation of show_all_overlays (for use by hotkeys and commands)
pub async fn show_all_overlays_impl(
    state: SharedOverlayState,
    service: crate::service::ServiceHandle,
) -> Result<Vec<MetricType>, String> {
    // Update and persist overlays_visible = true
    let mut config = service.config().await;
    config.overlay_settings.overlays_visible = true;
    service.update_config(config.clone()).await?;

    // Update state
    {
        let mut s = state.lock().map_err(|e| e.to_string())?;
        s.overlays_visible = true;
    }

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
    let mut needs_monitor_save: Vec<(String, tokio::sync::mpsc::Sender<OverlayCommand>)> = Vec::new();

    for key in &enabled_keys {
        if key == "personal" {
            let kind = OverlayType::Personal;
            let already_running = {
                let s = state.lock().map_err(|e| e.to_string())?;
                s.is_running(kind)
            };

            if !already_running {
                let position = config.overlay_settings.get_position("personal");
                let needs_save = position.monitor_id.is_none();
                let personal_config = config.overlay_settings.personal_overlay.clone();
                let overlay_handle = create_personal_overlay(position, personal_config, personal_opacity)?;
                let tx = overlay_handle.tx.clone();

                {
                    let mut s = state.lock().map_err(|e| e.to_string())?;
                    s.insert(overlay_handle);
                }

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
            let kind = OverlayType::Raid;
            let already_running = {
                let s = state.lock().map_err(|e| e.to_string())?;
                s.is_running(kind)
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
                    let mut s = state.lock().map_err(|e| e.to_string())?;
                    s.insert(overlay_handle);
                }

                if needs_save {
                    needs_monitor_save.push(("raid".to_string(), tx));
                }
            }
        } else if key == "boss_health" {
            let kind = OverlayType::BossHealth;
            let already_running = {
                let s = state.lock().map_err(|e| e.to_string())?;
                s.is_running(kind)
            };

            if !already_running {
                let position = config.overlay_settings.get_position("boss_health");
                let needs_save = position.monitor_id.is_none();
                let boss_config = config.overlay_settings.boss_health.clone();
                let overlay_handle = create_boss_health_overlay(position, boss_config, config.overlay_settings.boss_health_opacity)?;
                let tx = overlay_handle.tx.clone();

                {
                    let mut s = state.lock().map_err(|e| e.to_string())?;
                    s.insert(overlay_handle);
                }

                if needs_save {
                    needs_monitor_save.push(("boss_health".to_string(), tx));
                }
            }
        } else if let Some(overlay_type) = MetricType::from_config_key(key) {
            let kind = OverlayType::Metric(overlay_type);
            {
                let s = state.lock().map_err(|e| e.to_string())?;
                if s.is_running(kind) {
                    shown_metric_types.push(overlay_type);
                    continue;
                }
            }

            let position = config.overlay_settings.get_position(key);
            let needs_save = position.monitor_id.is_none();
            let appearance = super::get_appearance_for_type(&config.overlay_settings, overlay_type);
            let overlay_handle = create_metric_overlay(overlay_type, position, appearance, metric_opacity)?;
            let tx = overlay_handle.tx.clone();

            {
                let mut s = state.lock().map_err(|e| e.to_string())?;
                s.insert(overlay_handle);
            }

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
        } else if key == "boss_health" {
            // Handle boss health overlay
            let kind = OverlayType::BossHealth;
            let already_running = {
                let state = state.lock().map_err(|e| e.to_string())?;
                state.is_running(kind)
            };

            if !already_running {
                let position = config.overlay_settings.get_position("boss_health");
                let needs_save = position.monitor_id.is_none();
                let boss_config = config.overlay_settings.boss_health.clone();
                let overlay_handle = create_boss_health_overlay(position, boss_config, config.overlay_settings.boss_health_opacity)?;
                let tx = overlay_handle.tx.clone();

                {
                    let mut state = state.lock().map_err(|e| e.to_string())?;
                    state.insert(overlay_handle);
                }

                // Boss health overlay gets data via BossHealthUpdated channel

                if needs_save {
                    needs_monitor_save.push(("boss_health".to_string(), tx));
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
            // Raid not running - just return false, don't error
            eprintln!("[REARRANGE] Raid overlay not running, ignoring toggle");
            return Ok(false);
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
    let (running_metric_types, personal_running, raid_running, boss_health_running, move_mode, rearrange_mode) = {
        let state = state.lock().map_err(|e| e.to_string())?;
        (
            state.running_metric_types(),
            state.is_personal_running(),
            state.is_raid_running(),
            state.is_boss_health_running(),
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
    let boss_health_enabled = config.overlay_settings.is_enabled("boss_health");

    Ok(OverlayStatusResponse {
        running: running_metric_types,
        enabled,
        personal_running,
        personal_enabled,
        raid_running,
        raid_enabled,
        boss_health_running,
        boss_health_enabled,
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

    // ─────────────────────────────────────────────────────────────────────────
    // Handle Personal Overlay - enable/disable based on profile
    // ─────────────────────────────────────────────────────────────────────────
    let personal_enabled = config.overlay_settings.enabled.get("personal").copied().unwrap_or(false);
    let personal_running = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.is_running(OverlayType::Personal)
    };

    if personal_running && !personal_enabled {
        // Shut down personal overlay if running but not enabled in profile
        eprintln!("[REFRESH] Shutting down personal overlay (disabled in profile)");
        if let Ok(mut state_guard) = state.lock()
            && let Some(handle) = state_guard.remove(OverlayType::Personal) {
                let _ = handle.tx.try_send(OverlayCommand::Shutdown);
        }
    } else if !personal_running && personal_enabled {
        // Start personal overlay if not running but enabled in profile
        eprintln!("[REFRESH] Starting personal overlay (enabled in profile)");
        let position = config.overlay_settings.get_position("personal");
        let personal_config = config.overlay_settings.personal_overlay.clone();
        match create_personal_overlay(position, personal_config, personal_opacity) {
            Ok(handle) => {
                if let Ok(mut state_guard) = state.lock() {
                    state_guard.insert(handle);
                }
            }
            Err(e) => eprintln!("[REFRESH] Failed to create personal overlay: {}", e),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Handle Metric Overlays - enable/disable based on profile
    // ─────────────────────────────────────────────────────────────────────────
    for metric_type in MetricType::all() {
        let key = metric_type.config_key();
        let enabled = config.overlay_settings.enabled.get(key).copied().unwrap_or(false);
        let running = {
            let s = state.lock().map_err(|e| e.to_string())?;
            s.is_running(OverlayType::Metric(*metric_type))
        };

        if running && !enabled {
            // Shut down metric overlay if running but not enabled in profile
            eprintln!("[REFRESH] Shutting down {} overlay (disabled in profile)", key);
            if let Ok(mut state_guard) = state.lock()
                && let Some(handle) = state_guard.remove(OverlayType::Metric(*metric_type)) {
                    let _ = handle.tx.try_send(OverlayCommand::Shutdown);
            }
        } else if !running && enabled {
            // Start metric overlay if not running but enabled in profile
            eprintln!("[REFRESH] Starting {} overlay (enabled in profile)", key);
            let position = config.overlay_settings.get_position(key);
            let appearance = super::get_appearance_for_type(&config.overlay_settings, *metric_type);
            match create_metric_overlay(*metric_type, position, appearance, metric_opacity) {
                Ok(handle) => {
                    if let Ok(mut state_guard) = state.lock() {
                        state_guard.insert(handle);
                    }
                }
                Err(e) => eprintln!("[REFRESH] Failed to create {} overlay: {}", key, e),
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Handle Raid Overlay - always recreate to handle grid size changes
    // ─────────────────────────────────────────────────────────────────────────
    let raid_enabled_in_profile = config.overlay_settings.enabled.get("raid").copied().unwrap_or(false);

    // Check if raid was running and shut it down
    let raid_was_running = {
        let mut was_running = false;
        if let Ok(mut state_guard) = state.lock()
            && let Some(handle) = state_guard.remove(OverlayType::Raid) {
                eprintln!("[REFRESH] Shutting down raid overlay for refresh");
                let _ = handle.tx.try_send(OverlayCommand::Shutdown);
                was_running = true;
        }
        was_running
    };

    // Recreate raid if it was running OR if profile has it enabled
    if raid_was_running || raid_enabled_in_profile {
        eprintln!("[REFRESH] Recreating raid overlay (was_running={}, profile_enabled={})",
            raid_was_running, raid_enabled_in_profile);
        let position = config.overlay_settings.get_position("raid");
        let raid_settings = &config.overlay_settings.raid_overlay;
        let layout = RaidGridLayout::from_config(raid_settings);
        let raid_config: RaidOverlayConfig = raid_settings.clone().into();
        let raid_opacity = config.overlay_settings.raid_opacity;

        match create_raid_overlay(position, layout, raid_config, raid_opacity) {
            Ok(handle) => {
                if let Ok(mut state_guard) = state.lock() {
                    state_guard.insert(handle);
                    eprintln!("[REFRESH] Raid overlay created and inserted into state");
                }
            }
            Err(e) => {
                eprintln!("[REFRESH] Failed to create raid overlay: {}", e);
            }
        }
    } else {
        eprintln!("[REFRESH] Raid not running and not enabled in profile, skipping");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Handle Boss Health Overlay - enable/disable based on profile
    // ─────────────────────────────────────────────────────────────────────────
    let boss_health_enabled = config.overlay_settings.enabled.get("boss_health").copied().unwrap_or(false);
    let boss_health_running = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.is_running(OverlayType::BossHealth)
    };

    if boss_health_running && !boss_health_enabled {
        eprintln!("[REFRESH] Shutting down boss health overlay (disabled in profile)");
        if let Ok(mut state_guard) = state.lock()
            && let Some(handle) = state_guard.remove(OverlayType::BossHealth) {
                let _ = handle.tx.try_send(OverlayCommand::Shutdown);
        }
    } else if !boss_health_running && boss_health_enabled {
        eprintln!("[REFRESH] Starting boss health overlay (enabled in profile)");
        let position = config.overlay_settings.get_position("boss_health");
        let boss_config = config.overlay_settings.boss_health.clone();
        match create_boss_health_overlay(position, boss_config, config.overlay_settings.boss_health_opacity) {
            Ok(handle) => {
                if let Ok(mut state_guard) = state.lock() {
                    state_guard.insert(handle);
                }
            }
            Err(e) => eprintln!("[REFRESH] Failed to create boss health overlay: {}", e),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Update config for all currently running overlays
    // ─────────────────────────────────────────────────────────────────────────
    let overlays: Vec<_> = {
        let state = state.lock().map_err(|e| e.to_string())?;
        state.all_overlays().into_iter().map(|(k, tx)| (k, tx.clone())).collect()
    };

    // Send updated config and position to each overlay based on its type
    for (kind, tx) in overlays {
        // Get the config key for this overlay type
        let config_key = match kind {
            OverlayType::Metric(overlay_type) => overlay_type.config_key().to_string(),
            OverlayType::Personal => "personal".to_string(),
            OverlayType::Raid => "raid".to_string(),
            OverlayType::BossHealth => "boss_health".to_string(),
        };

        // Send position update if we have saved position for this overlay
        if let Some(pos) = config.overlay_settings.positions.get(&config_key) {
            eprintln!("[REFRESH] Updating {} position to ({}, {})", config_key, pos.x, pos.y);
            let _ = tx.send(OverlayCommand::SetPosition(pos.x, pos.y)).await;
        }

        // Send config update
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
            OverlayType::BossHealth => {
                let boss_config = config.overlay_settings.boss_health.clone();
                OverlayConfigUpdate::BossHealth(boss_config, config.overlay_settings.boss_health_opacity)
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
