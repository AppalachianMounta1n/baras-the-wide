//! Metric entry creation helpers
//!
//! Functions for converting player metrics into overlay entries.

use std::collections::HashMap;

use baras_overlay::MeterEntry;

use super::types::MetricType;
use crate::service::PlayerMetrics;

/// Create meter entries for a specific overlay type from player metrics
pub fn create_entries_for_type(overlay_type: MetricType, metrics: &[PlayerMetrics]) -> Vec<MeterEntry> {
    let color = overlay_type.bar_color();

    // Extract (name, rate_value, total_value) tuples based on metric type
    let mut values: Vec<(String, i64, i64)> = match overlay_type {
        MetricType::Dps => metrics
            .iter()
            .map(|m| (m.name.clone(), m.dps, m.total_damage as i64))
            .collect(),
        MetricType::EDps => metrics
            .iter()
            .map(|m| (m.name.clone(), m.edps, m.total_damage_effective as i64))
            .collect(),
        MetricType::Hps => metrics
            .iter()
            .map(|m| (m.name.clone(), m.hps, m.total_healing as i64))
            .collect(),
        MetricType::EHps => metrics
            .iter()
            .map(|m| (m.name.clone(), m.ehps, m.total_healing_effective as i64))
            .collect(),
        MetricType::Tps => metrics
            .iter()
            .map(|m| (m.name.clone(), m.tps, m.total_threat as i64))
            .collect(),
        MetricType::Dtps => metrics
            .iter()
            .map(|m| (m.name.clone(), m.dtps, m.total_damage_taken as i64))
            .collect(),
        MetricType::EDtps => metrics
            .iter()
            .map(|m| (m.name.clone(), m.edtps, m.total_damage_taken_effective as i64))
            .collect(),
        MetricType::Abs => metrics
            .iter()
            .map(|m| (m.name.clone(), m.abs, m.total_shielding as i64))
            .collect(),
    };

    // Sort by rate value descending (highest first)
    values.sort_by(|a, b| b.1.cmp(&a.1));

    // Find max rate for progress bar scaling
    let max_value = values.iter().map(|(_, rate, _)| *rate).max().unwrap_or(1);

    values
        .into_iter()
        .map(|(name, rate, total)| {
            MeterEntry::new(&name, rate, max_value)
                .with_total(total)
                .with_color(color)
        })
        .collect()
}

/// Create entries for all overlay types from metrics
pub fn create_all_entries(metrics: &[PlayerMetrics]) -> HashMap<MetricType, Vec<MeterEntry>> {
    let mut result = HashMap::new();
    for overlay_type in MetricType::all() {
        result.insert(*overlay_type, create_entries_for_type(*overlay_type, metrics));
    }
    result
}
