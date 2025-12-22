use crate::overlay::{create_all_entries, OverlayCommand, OverlayType, MetricType};
use crate::service::{OverlayUpdate, ServiceHandle};
use crate::SharedOverlayState;
use baras_overlay::{OverlayData, RaidRegistryAction};
use tokio::sync::mpsc;

/// Bridge between service overlay updates and the overlay threads
///
/// Also polls the raid overlay's registry action channel and forwards
/// swap/clear commands to the service registry.
pub fn spawn_overlay_bridge(
    mut rx: mpsc::Receiver<OverlayUpdate>,
    overlay_state: SharedOverlayState,
    service_handle: ServiceHandle,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            // Check for overlay updates (non-blocking with timeout for polling)
            let update = tokio::time::timeout(
                std::time::Duration::from_millis(50),
                rx.recv()
            ).await;

            // Process overlay update if received
            match update {
                Ok(Some(update)) => {
                    process_overlay_update(&overlay_state, update).await;
                }
                Ok(None) => {
                    // Channel closed
                    break;
                }
                Err(_) => {
                    // Timeout - no update received, continue to poll registry actions
                }
            }

            // Poll raid overlay's registry action channel
            poll_registry_actions(&overlay_state, &service_handle).await;
        }
    });
}

/// Process a single overlay update
async fn process_overlay_update(overlay_state: &SharedOverlayState, update: OverlayUpdate) {
    match update {
        OverlayUpdate::DataUpdated(data) => {
            // Create entries for all metric overlay types
            let all_entries = create_all_entries(&data.metrics);

            // Get running metric overlays and their channels
            let (metric_txs, personal_tx): (Vec<_>, _) = {
                let state = match overlay_state.lock() {
                    Ok(s) => s,
                    Err(_) => return,
                };

                let metric_txs = MetricType::all()
                    .iter()
                    .filter_map(|&overlay_type| {
                        let kind = OverlayType::Metric(overlay_type);
                        state.get_tx(kind).cloned().map(|tx| (overlay_type, tx))
                    })
                    .collect();

                let personal_tx = state.get_personal_tx().cloned();

                (metric_txs, personal_tx)
            };

            // Send entries to each running metric overlay
            for (overlay_type, tx) in metric_txs {
                if let Some(entries) = all_entries.get(&overlay_type) {
                    let _ = tx.send(OverlayCommand::UpdateData(
                        OverlayData::Metrics(entries.clone())
                    )).await;
                }
            }

            // Send personal stats to personal overlay
            if let Some(tx) = personal_tx
                && let Some(stats) = data.to_personal_stats()
            {
                let _ = tx.send(OverlayCommand::UpdateData(
                    OverlayData::Personal(stats)
                )).await;
            }
        }
        OverlayUpdate::EffectsUpdated(raid_data) => {
            // Send raid frame data to raid overlay
            let raid_tx = {
                let state = match overlay_state.lock() {
                    Ok(s) => s,
                    Err(_) => return,
                };
                state.get_raid_tx().cloned()
            };

            if let Some(tx) = raid_tx {
                let _ = tx.send(OverlayCommand::UpdateData(
                    OverlayData::Raid(raid_data)
                )).await;
            }
        }
        OverlayUpdate::CombatStarted => {
            // Could show overlay or clear entries
        }
        OverlayUpdate::CombatEnded => {
            // Could hide overlay or freeze display
        }
    }
}

/// Poll the raid overlay's registry action channel and forward to service
async fn poll_registry_actions(overlay_state: &SharedOverlayState, service_handle: &ServiceHandle) {
    // Get the registry action receiver from the raid overlay handle
    let actions: Vec<RaidRegistryAction> = {
        let state = match overlay_state.lock() {
            Ok(s) => s,
            Err(_) => return,
        };

        // Try to get actions from the raid overlay's registry channel
        if let Some(handle) = state.overlays.get(&OverlayType::Raid) {
            if let Some(ref rx) = handle.registry_action_rx {
                // Drain all pending actions (non-blocking)
                let mut actions = Vec::new();
                while let Ok(action) = rx.try_recv() {
                    actions.push(action);
                }
                actions
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    };

    // Process each action
    for action in actions {
        match action {
            RaidRegistryAction::SwapSlots(a, b) => {
                eprintln!("[BRIDGE] Processing SwapSlots({}, {})", a, b);
                service_handle.swap_raid_slots(a, b).await;
            }
            RaidRegistryAction::ClearSlot(slot) => {
                eprintln!("[BRIDGE] Processing ClearSlot({})", slot);
                service_handle.remove_raid_slot(slot).await;
            }
        }
    }
}
