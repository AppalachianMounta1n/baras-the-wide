use crate::overlay::MetricType;
use crate::service::CombatData;
use crate::service::LogFileInfo;
use crate::service::SharedState;
use crate::service::PlayerMetrics;
use crate::service::SessionInfo;
use crate::service::ServiceCommand;
use std::sync::atomic::Ordering;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use baras_core::context::{resolve, AppConfig, OverlayAppearanceConfig};
use baras_core::encounter::EncounterState;
use baras_core::EntityType;

// ─────────────────────────────────────────────────────────────────────────────
// Service Handle (for Tauri commands)
// ─────────────────────────────────────────────────────────────────────────────

/// Handle to communicate with the combat service and query state
#[derive(Clone)]
pub struct ServiceHandle {
    pub cmd_tx: mpsc::Sender<ServiceCommand>,
    pub shared: Arc<SharedState>,
}

impl ServiceHandle {
    /// Send command to start tailing a log file
    pub async fn start_tailing(&self, path: PathBuf) -> Result<(), String> {
        self.cmd_tx
            .send(ServiceCommand::StartTailing(path))
            .await
            .map_err(|e| e.to_string())
    }

    /// Send command to stop tailing
    pub async fn stop_tailing(&self) -> Result<(), String> {
        self.cmd_tx
            .send(ServiceCommand::StopTailing)
            .await
            .map_err(|e| e.to_string())
    }

    /// Send command to refresh the directory index
    pub async fn refresh_index(&self) -> Result<(), String> {
        self.cmd_tx
            .send(ServiceCommand::RefreshIndex)
            .await
            .map_err(|e| e.to_string())
    }

    /// Get the current configuration
    pub async fn config(&self) -> AppConfig {
        self.shared.config.read().await.clone()
    }

    /// Update the configuration
    pub async fn update_config(&self, config: AppConfig) -> Result<(), String> {
        let old_dir = self.shared.config.read().await.log_directory.clone();
        let new_dir = config.log_directory.clone();
        *self.shared.config.write().await = config.clone();
        config.save();

        if old_dir != new_dir {
            self.cmd_tx
                .send(ServiceCommand::DirectoryChanged)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Get log file entries for the UI
    pub async fn log_files(&self) -> Vec<LogFileInfo> {
        let index = self.shared.directory_index.read().await;
        index
            .entries()
            .into_iter()
            .map(|e| LogFileInfo {
                path: e.path.clone(),
                display_name: e.display_name(),
                character_name: e.character_name.clone(),
                date: e.date.to_string(),
                is_empty: e.is_empty,
            })
            .collect()
    }

    /// Check if currently tailing a file
    pub async fn is_tailing(&self) -> bool {
        self.shared.session.read().await.is_some()
    }

    pub async fn active_file(&self) -> Option<String> {
        self.shared.with_session(|session|
            { session.active_file.as_ref().map(|p| p.to_string_lossy().to_string())})
            .await
            .unwrap_or(Some("None".to_string()))
    }

    /// Get current session info
    pub async fn session_info(&self) -> Option<SessionInfo> {
        let session_guard = self.shared.session.read().await;
        let session = session_guard.as_ref()?;
        let session = session.read().await;
        let cache = session.session_cache.as_ref()?;

        Some(SessionInfo {
            player_name: if cache.player_initialized {
                Some(resolve(cache.player.name).to_string())
            } else {
                None
            },
            player_class: if cache.player_initialized {
                Some(cache.player.class_name.clone())
            } else {
                None
            },
            player_discipline: if cache.player_initialized {
                Some(cache.player.discipline_name.clone())
            } else {
                None
            },
            area_name: if !cache.current_area.area_name.is_empty() {
                Some(cache.current_area.area_name.clone())
            } else {
                None
            },
            in_combat: self.shared.in_combat.load(Ordering::SeqCst),
            encounter_count: cache.encounters().filter(|e| e.state != EncounterState::NotStarted ).map(|e| e.id + 1).max().unwrap_or(0) as usize
        })
    }

    /// Get current combat data (unified for all overlays)
    pub async fn current_combat_data(&self) -> Option<CombatData> {
        let session_guard = self.shared.session.read().await;
        let session = session_guard.as_ref()?;
        let session = session.read().await;
        let cache = session.session_cache.as_ref()?;

        // Get player info
        let player_info = &cache.player;
        let class_discipline = if !player_info.class_name.is_empty() && !player_info.discipline_name.is_empty() {
            Some(format!("{} / {}", player_info.class_name, player_info.discipline_name))
        } else if !player_info.class_name.is_empty() {
            Some(player_info.class_name.clone())
        } else {
            None
        };
        let player_entity_id = player_info.id;

        // Get encounter info
        let encounter = cache.last_combat_encounter()?;
        let encounter_count = cache.encounter_count();
        let encounter_time_secs = (encounter.duration_ms().unwrap_or(0) / 1000) as u64;

        // Calculate metrics
        let entity_metrics = encounter.calculate_entity_metrics()?;
        let metrics: Vec<PlayerMetrics> = entity_metrics
            .into_iter()
            .filter(|m| m.entity_type != EntityType::Npc)
            .map(|m| {
                let name = resolve(m.name).to_string();
                let safe_name: String = name.chars().filter(|c| !c.is_control()).collect();
                PlayerMetrics {
                    entity_id: m.entity_id,
                    name: safe_name,
                    dps: m.dps as i64,
                    edps: m.edps as i64,
                    total_damage: m.total_damage as u64,
                    total_damage_effective: m.total_damage_effective as u64,
                    damage_crit_pct: m.damage_crit_pct,
                    hps: m.hps as i64,
                    ehps: m.ehps as i64,
                    total_healing: m.total_healing as u64,
                    total_healing_effective: m.total_healing_effective as u64,
                    heal_crit_pct: m.heal_crit_pct,
                    effective_heal_pct: m.effective_heal_pct,
                    tps: m.tps as i64,
                    total_threat: m.total_threat as u64,
                    dtps: m.dtps as i64,
                    edtps: m.edtps as i64,
                    total_damage_taken: m.total_damage_taken as u64,
                    total_damage_taken_effective: m.total_damage_taken_effective as u64,
                    abs: m.abs as i64,
                    total_shielding: m.total_shielding as u64,
                    apm: m.apm,
                }
            })
            .collect();

        Some(CombatData {
            metrics,
            player_entity_id,
            encounter_time_secs,
            encounter_count,
            class_discipline,
        })
    }
}



// ─────────────────────────────────────────────────────────────────────────────
// Tauri Commands
// ─────────────────────────────────────────────────────────────────────────────

use tauri::State;

#[tauri::command]
pub async fn get_log_files(handle: State<'_, ServiceHandle>) -> Result<Vec<LogFileInfo>, String> {
    Ok(handle.log_files().await)
}

#[tauri::command]
pub async fn start_tailing(path: PathBuf, handle: State<'_, ServiceHandle>) -> Result<(), String> {
    handle.start_tailing(path).await
}

#[tauri::command]
pub async fn stop_tailing(handle: State<'_, ServiceHandle>) -> Result<(), String> {
    handle.stop_tailing().await
}

#[tauri::command]
pub async fn refresh_log_index(handle: State<'_, ServiceHandle>) -> Result<(), String> {
    handle.refresh_index().await
}

#[tauri::command]
pub async fn get_tailing_status(handle: State<'_, ServiceHandle>) -> Result<bool, String> {
    Ok(handle.is_tailing().await)
}

#[tauri::command]
pub async fn get_current_metrics(
    handle: State<'_, ServiceHandle>,
) -> Result<Option<Vec<PlayerMetrics>>, String> {
    Ok(handle.current_combat_data().await.map(|d| d.metrics))
}

#[tauri::command]
pub async fn get_config(handle: State<'_, ServiceHandle>) -> Result<AppConfig, String> {
    let mut config = handle.config().await;

    // Populate default appearances for each overlay type (single source of truth)
    for metric_type in MetricType::all() {
        let color = metric_type.bar_color();
        // Convert from f32 (0.0-1.0) to u8 (0-255)
        let bar_color = [
            (color.red() * 255.0) as u8,
            (color.green() * 255.0) as u8,
            (color.blue() * 255.0) as u8,
            (color.alpha() * 255.0) as u8,
        ];
        config.overlay_settings.default_appearances.insert(
            metric_type.config_key().to_string(),
            OverlayAppearanceConfig {
                bar_color,
                ..Default::default()
            },
        );
    }

    Ok(config)
}

#[tauri::command]
pub async fn get_active_file(handle: State<'_, ServiceHandle>) -> Result<Option<String>, String> {
    Ok(handle.active_file().await)
}

#[tauri::command]
pub async fn update_config(
    config: AppConfig,
    handle: State<'_, ServiceHandle>
) -> Result<(), String> {
    handle.update_config(config).await
}

#[tauri::command]
pub async fn get_session_info(
    handle: State<'_, ServiceHandle>
) -> Result<Option<SessionInfo>, String> {
    Ok(handle.session_info().await)
}
