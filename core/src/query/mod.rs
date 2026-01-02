//! Query module for analyzing encounter data with DataFusion.
//!
//! Provides SQL queries over:
//! - Live Arrow buffers (current encounter)
//! - Historical parquet files (completed encounters)

use std::path::Path;
use std::sync::Arc;

use arrow::array::{
    Array, Float32Array, Float64Array, Int32Array, Int64Array, LargeStringArray, StringArray,
    StringViewArray, UInt64Array,
};
use arrow::record_batch::RecordBatch;
use datafusion::datasource::MemTable;
use datafusion::prelude::*;

use crate::storage::EncounterWriter;

// Re-export query types from shared types crate
pub use baras_types::{
    AbilityBreakdown, EncounterTimeline, EntityBreakdown, PhaseSegment, TimeRange, TimeSeriesPoint,
};

/// Escape single quotes for SQL string literals (O'Brien -> O''Brien)
fn sql_escape(s: &str) -> String {
    s.replace('\'', "''")
}

// ─────────────────────────────────────────────────────────────────────────────
// Generic Column Extractors (handles Arrow type variations automatically)
// ─────────────────────────────────────────────────────────────────────────────

fn col_strings(batch: &RecordBatch, idx: usize) -> Result<Vec<String>, String> {
    let col = batch.column(idx);
    if let Some(a) = col.as_any().downcast_ref::<StringViewArray>() {
        return Ok((0..a.len()).map(|i| a.value(i).to_string()).collect());
    }
    if let Some(a) = col.as_any().downcast_ref::<StringArray>() {
        return Ok((0..a.len()).map(|i| a.value(i).to_string()).collect());
    }
    if let Some(a) = col.as_any().downcast_ref::<LargeStringArray>() {
        return Ok((0..a.len()).map(|i| a.value(i).to_string()).collect());
    }
    Err(format!("col {idx}: expected string, got {:?}", col.data_type()))
}

fn col_i64(batch: &RecordBatch, idx: usize) -> Result<Vec<i64>, String> {
    let col = batch.column(idx);
    if let Some(a) = col.as_any().downcast_ref::<Int64Array>() {
        return Ok((0..a.len()).map(|i| a.value(i)).collect());
    }
    if let Some(a) = col.as_any().downcast_ref::<Int32Array>() {
        return Ok((0..a.len()).map(|i| a.value(i) as i64).collect());
    }
    if let Some(a) = col.as_any().downcast_ref::<UInt64Array>() {
        return Ok((0..a.len()).map(|i| a.value(i) as i64).collect());
    }
    Err(format!("col {idx}: expected int, got {:?}", col.data_type()))
}

fn col_f64(batch: &RecordBatch, idx: usize) -> Result<Vec<f64>, String> {
    let col = batch.column(idx);
    if let Some(a) = col.as_any().downcast_ref::<Float64Array>() {
        return Ok((0..a.len()).map(|i| a.value(i)).collect());
    }
    if let Some(a) = col.as_any().downcast_ref::<Float32Array>() {
        return Ok((0..a.len()).map(|i| a.value(i) as f64).collect());
    }
    if let Some(a) = col.as_any().downcast_ref::<Int64Array>() {
        return Ok((0..a.len()).map(|i| a.value(i) as f64).collect());
    }
    if let Some(a) = col.as_any().downcast_ref::<Int32Array>() {
        return Ok((0..a.len()).map(|i| a.value(i) as f64).collect());
    }
    Err(format!("col {idx}: expected float, got {:?}", col.data_type()))
}

fn col_f32(batch: &RecordBatch, idx: usize) -> Result<Vec<f32>, String> {
    let col = batch.column(idx);
    if let Some(a) = col.as_any().downcast_ref::<Float32Array>() {
        return Ok((0..a.len()).map(|i| a.value(i)).collect());
    }
    if let Some(a) = col.as_any().downcast_ref::<Float64Array>() {
        return Ok((0..a.len()).map(|i| a.value(i) as f32).collect());
    }
    Err(format!("col {idx}: expected float, got {:?}", col.data_type()))
}

fn scalar_f32(batches: &[RecordBatch]) -> f32 {
    batches.first().and_then(|b| {
        if b.num_rows() == 0 { return None; }
        col_f32(b, 0).ok().and_then(|v| v.first().copied())
    }).unwrap_or(0.0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Query Executor
// ─────────────────────────────────────────────────────────────────────────────

pub struct EncounterQuery {
    ctx: SessionContext,
}

impl Default for EncounterQuery {
    fn default() -> Self {
        Self::new()
    }
}

impl EncounterQuery {
    pub fn new() -> Self {
        Self { ctx: SessionContext::new() }
    }

    pub async fn register_live(&self, writer: &EncounterWriter) -> Result<(), String> {
        let batch = writer.to_record_batch().ok_or("No data in live buffer")?;
        self.register_batch(batch).await
    }

    pub async fn register_batch(&self, batch: RecordBatch) -> Result<(), String> {
        let schema = batch.schema();
        let mem_table = MemTable::try_new(schema, vec![vec![batch]]).map_err(|e| e.to_string())?;
        self.ctx.register_table("events", Arc::new(mem_table)).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn register_parquet(&self, path: &Path) -> Result<(), String> {
        self.ctx
            .register_parquet("events", path.to_string_lossy().as_ref(), ParquetReadOptions::default())
            .await
            .map_err(|e| e.to_string())
    }

    async fn sql(&self, query: &str) -> Result<Vec<RecordBatch>, String> {
        let df = self.ctx.sql(query).await.map_err(|e| e.to_string())?;
        df.collect().await.map_err(|e| e.to_string())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Query Methods
    // ─────────────────────────────────────────────────────────────────────────

    pub async fn damage_by_ability(&self, source_name: Option<&str>, time_range: Option<&TimeRange>) -> Result<Vec<AbilityBreakdown>, String> {
        let mut conditions = vec!["dmg_amount > 0".to_string()];
        if let Some(n) = source_name {
            conditions.push(format!("source_name = '{}'", sql_escape(n)));
        }
        if let Some(tr) = time_range {
            conditions.push(tr.sql_filter());
        }
        let filter = format!("WHERE {}", conditions.join(" AND "));

        let batches = self.sql(&format!(r#"
            SELECT ability_name, ability_id,
                   SUM(dmg_amount) as total_value, COUNT(*) as hit_count,
                   SUM(CASE WHEN is_crit THEN 1 ELSE 0 END) as crit_count,
                   MAX(dmg_amount) as max_hit
            FROM events {filter}
            GROUP BY ability_name, ability_id
            ORDER BY total_value DESC
        "#)).await?;

        let mut results = Vec::new();
        for batch in &batches {
            let names = col_strings(batch, 0)?;
            let ids = col_i64(batch, 1)?;
            let totals = col_f64(batch, 2)?;
            let hits = col_i64(batch, 3)?;
            let crits = col_i64(batch, 4)?;
            let maxes = col_f64(batch, 5)?;

            for i in 0..batch.num_rows() {
                let h = hits[i] as f64;
                results.push(AbilityBreakdown {
                    ability_name: names[i].clone(),
                    ability_id: ids[i],
                    total_value: totals[i],
                    hit_count: hits[i],
                    crit_count: crits[i],
                    crit_rate: if h > 0.0 { crits[i] as f64 / h * 100.0 } else { 0.0 },
                    max_hit: maxes[i],
                    avg_hit: if h > 0.0 { totals[i] / h } else { 0.0 },
                });
            }
        }
        Ok(results)
    }

    pub async fn breakdown_by_entity(&self, time_range: Option<&TimeRange>) -> Result<Vec<EntityBreakdown>, String> {
        let mut conditions = vec!["dmg_amount > 0".to_string()];
        if let Some(tr) = time_range {
            conditions.push(tr.sql_filter());
        }
        let filter = format!("WHERE {}", conditions.join(" AND "));

        let batches = self.sql(&format!(r#"
            SELECT source_name, source_id, SUM(dmg_amount) as total_value,
                   COUNT(DISTINCT ability_id) as abilities_used
            FROM events {filter}
            GROUP BY source_name, source_id
            ORDER BY total_value DESC
        "#)).await?;

        let mut results = Vec::new();
        for batch in &batches {
            let names = col_strings(batch, 0)?;
            let ids = col_i64(batch, 1)?;
            let totals = col_f64(batch, 2)?;
            let abilities = col_i64(batch, 3)?;

            for i in 0..batch.num_rows() {
                results.push(EntityBreakdown {
                    source_name: names[i].clone(),
                    source_id: ids[i],
                    total_value: totals[i],
                    abilities_used: abilities[i],
                });
            }
        }
        Ok(results)
    }

    pub async fn dps_over_time(&self, bucket_ms: i64, source_name: Option<&str>, time_range: Option<&TimeRange>) -> Result<Vec<TimeSeriesPoint>, String> {
        let mut conditions = vec!["dmg_amount > 0".to_string()];
        if let Some(n) = source_name {
            conditions.push(format!("source_name = '{}'", sql_escape(n)));
        }
        if let Some(tr) = time_range {
            conditions.push(tr.sql_filter());
        }
        let filter = format!("WHERE {}", conditions.join(" AND "));

        let batches = self.sql(&format!(r#"
            SELECT (CAST(timestamp AS BIGINT) / {bucket_ms}) * {bucket_ms} as bucket_start_ms,
                   SUM(dmg_amount) as total_value
            FROM events {filter}
            GROUP BY bucket_start_ms ORDER BY bucket_start_ms
        "#)).await?;

        let mut results = Vec::new();
        for batch in &batches {
            let buckets = col_i64(batch, 0)?;
            let values = col_f64(batch, 1)?;
            for i in 0..batch.num_rows() {
                results.push(TimeSeriesPoint { bucket_start_ms: buckets[i], total_value: values[i] });
            }
        }
        Ok(results)
    }

    /// Get encounter timeline with phase segments (handles repeated phases).
    pub async fn encounter_timeline(&self) -> Result<EncounterTimeline, String> {
        let duration_secs = scalar_f32(&self.sql(
            "SELECT COALESCE(MAX(combat_time_secs), 0) FROM events WHERE combat_time_secs IS NOT NULL"
        ).await?);

        // Window functions to detect phase transitions and number instances
        // Filter: phase_id must be non-null AND non-empty string
        let batches = self.sql(r#"
            WITH filtered AS (
                SELECT combat_time_secs, phase_id, phase_name
                FROM events
                WHERE phase_id IS NOT NULL
                  AND phase_id != ''
                  AND combat_time_secs IS NOT NULL
            ),
            transitions AS (
                SELECT combat_time_secs, phase_id, phase_name,
                       CASE WHEN phase_id != LAG(phase_id) OVER (ORDER BY combat_time_secs)
                                 OR LAG(phase_id) OVER (ORDER BY combat_time_secs) IS NULL
                            THEN 1 ELSE 0 END as is_new
                FROM filtered
            ),
            segments AS (
                SELECT *, SUM(is_new) OVER (ORDER BY combat_time_secs) as seg_id FROM transitions
            ),
            bounds AS (
                SELECT phase_id, phase_name, seg_id,
                       MIN(combat_time_secs) as start_secs, MAX(combat_time_secs) as end_secs
                FROM segments GROUP BY phase_id, phase_name, seg_id
            ),
            valid_bounds AS (
                SELECT * FROM bounds WHERE start_secs < end_secs
            )
            SELECT phase_id, phase_name,
                   ROW_NUMBER() OVER (PARTITION BY phase_id ORDER BY seg_id) as instance,
                   start_secs, end_secs
            FROM valid_bounds
            ORDER BY start_secs
        "#).await?;

        let mut phases = Vec::new();
        for batch in &batches {
            let ids = col_strings(batch, 0)?;
            let names = col_strings(batch, 1)?;
            let instances = col_i64(batch, 2)?;
            let starts = col_f32(batch, 3)?;
            let ends = col_f32(batch, 4)?;

            for i in 0..batch.num_rows() {
                phases.push(PhaseSegment {
                    phase_id: ids[i].clone(),
                    phase_name: names[i].clone(),
                    instance: instances[i],
                    start_secs: starts[i],
                    end_secs: ends[i],
                });
            }
        }

        Ok(EncounterTimeline { duration_secs, phases })
    }
}
