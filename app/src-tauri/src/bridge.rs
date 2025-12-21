use crate::overlay::{create_all_entries, OverlayCommand, OverlayType, MetricType};
use crate::service::OverlayUpdate;
use crate::SharedOverlayState;
use baras_overlay::OverlayData;
use tokio::sync::mpsc;

/// Bridge between service overlay updates and the overlay threads
pub fn spawn_overlay_bridge(
    mut rx: mpsc::Receiver<OverlayUpdate>,
    overlay_state: SharedOverlayState,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(update) = rx.recv().await {
            match update {
                OverlayUpdate::DataUpdated(data) => {
                    // Create entries for all metric overlay types
                    let all_entries = create_all_entries(&data.metrics);

                    // Get running metric overlays and their channels
                    let (metric_txs, personal_tx): (Vec<_>, _) = {
                        let state = match overlay_state.lock() {
                            Ok(s) => s,
                            Err(_) => continue,
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
                OverlayUpdate::CombatStarted => {
                    // Could show overlay or clear entries
                }
                OverlayUpdate::CombatEnded => {
                    // Could hide overlay or freeze display
                }
            }
        }
    });
}
