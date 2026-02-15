//! Time series queries (DPS, HPS, DTPS over time).

use super::*;

/// Configuration for time series queries.
struct TimeSeriesConfig<'a> {
    /// Column to sum ("dmg_amount" or "heal_amount")
    value_column: &'static str,
    /// Column to filter by entity ("source_name" or "target_name")
    entity_column: &'static str,
    /// Optional entity name filter
    entity_filter: Option<&'a str>,
}

impl EncounterQuery<'_> {
    /// Generic time series query - buckets values over time with optional entity filter.
    async fn query_time_series(
        &self,
        bucket_ms: i64,
        config: TimeSeriesConfig<'_>,
        time_range: Option<&TimeRange>,
    ) -> Result<Vec<TimeSeriesPoint>, String> {
        let bucket_secs = (bucket_ms as f64 / 1000.0).max(1.0);
        let value_col = config.value_column;
        let entity_col = config.entity_column;

        // Base conditions for time range (used for bounds calculation)
        let mut tr_conditions = vec!["combat_time_secs IS NOT NULL".to_string()];
        if let Some(tr) = time_range {
            tr_conditions.push(tr.sql_filter());
        }
        let tr_filter = format!("WHERE {}", tr_conditions.join(" AND "));

        // Entity-specific conditions (used for value aggregation)
        let mut entity_conditions = tr_conditions.clone();
        if let Some(name) = config.entity_filter {
            entity_conditions.push(format!("{} = '{}'", entity_col, sql_escape(name)));
        }
        let entity_filter = format!("WHERE {}", entity_conditions.join(" AND "));

        let batches = self
            .sql(&format!(
                r#"
WITH bounds AS (
    SELECT
        CAST(MIN(FLOOR(combat_time_secs / {bucket_secs})) as BIGINT) as min_bucket,
        CAST(MAX(FLOOR(combat_time_secs / {bucket_secs})) as BIGINT) as max_bucket
    FROM events
    {tr_filter}
),
time_series AS (
    SELECT
      unnest(generate_series(bounds.min_bucket, bounds.max_bucket, 1)) * {bucket_secs} * 1000 AS bucket_start_ms
    FROM bounds
),
entity_ts AS (
    SELECT CAST(FLOOR(combat_time_secs / {bucket_secs}) * {bucket_secs} * 1000 AS BIGINT) as bucket_start_ms,
           SUM({value_col}) as total_value
    FROM events
    {entity_filter}
    GROUP BY bucket_start_ms
)
SELECT
    time_series.bucket_start_ms,
    COALESCE(entity_ts.total_value, 0) as total_value
FROM time_series
LEFT JOIN entity_ts ON time_series.bucket_start_ms = entity_ts.bucket_start_ms
ORDER BY time_series.bucket_start_ms
            "#
            ))
            .await?;

        let mut results = Vec::new();
        for batch in &batches {
            let buckets = col_i64(batch, 0)?;
            let values = col_f64(batch, 1)?;
            for i in 0..batch.num_rows() {
                results.push(TimeSeriesPoint {
                    bucket_start_ms: buckets[i],
                    total_value: values[i],
                });
            }
        }
        Ok(results)
    }

    /// Query DPS (damage per second) over time, bucketed by time interval.
    pub async fn dps_over_time(
        &self,
        bucket_ms: i64,
        source_name: Option<&str>,
        time_range: Option<&TimeRange>,
    ) -> Result<Vec<TimeSeriesPoint>, String> {
        self.query_time_series(
            bucket_ms,
            TimeSeriesConfig {
                value_column: "dmg_amount",
                entity_column: "source_name",
                entity_filter: source_name,
            },
            time_range,
        )
        .await
    }

    /// Query HPS (healing per second) over time, bucketed by time interval.
    pub async fn hps_over_time(
        &self,
        bucket_ms: i64,
        source_name: Option<&str>,
        time_range: Option<&TimeRange>,
    ) -> Result<Vec<TimeSeriesPoint>, String> {
        self.query_time_series(
            bucket_ms,
            TimeSeriesConfig {
                value_column: "heal_amount",
                entity_column: "source_name",
                entity_filter: source_name,
            },
            time_range,
        )
        .await
    }

    /// Query EHPS (effective healing per second) over time, bucketed by time interval.
    pub async fn ehps_over_time(
        &self,
        bucket_ms: i64,
        source_name: Option<&str>,
        time_range: Option<&TimeRange>,
    ) -> Result<Vec<TimeSeriesPoint>, String> {
        self.query_time_series(
            bucket_ms,
            TimeSeriesConfig {
                value_column: "heal_effective",
                entity_column: "source_name",
                entity_filter: source_name,
            },
            time_range,
        )
        .await
    }

    /// Query DTPS (damage taken per second) over time for a target entity.
    pub async fn dtps_over_time(
        &self,
        bucket_ms: i64,
        target_name: Option<&str>,
        time_range: Option<&TimeRange>,
    ) -> Result<Vec<TimeSeriesPoint>, String> {
        self.query_time_series(
            bucket_ms,
            TimeSeriesConfig {
                value_column: "dmg_amount",
                entity_column: "target_name",
                entity_filter: target_name,
            },
            time_range,
        )
        .await
    }

    /// Query HP% over time for an entity, taking the last HP% per bucket.
    ///
    /// Picks the last event per time bucket (by line_number) and returns
    /// hp_pct, current_hp, max_hp. Missing buckets are forward-filled with
    /// the last known value to avoid fake-zero spikes.
    pub async fn hp_over_time(
        &self,
        bucket_ms: i64,
        entity_name: Option<&str>,
        time_range: Option<&TimeRange>,
    ) -> Result<Vec<HpPoint>, String> {
        use datafusion::arrow::array::{Float64Array, Int32Array, Int64Array};

        let bucket_secs = (bucket_ms as f64 / 1000.0).max(1.0);

        let mut tr_conditions = vec!["combat_time_secs IS NOT NULL".to_string()];
        if let Some(tr) = time_range {
            tr_conditions.push(tr.sql_filter());
        }
        let tr_filter = format!("WHERE {}", tr_conditions.join(" AND "));

        let mut entity_conditions = tr_conditions.clone();
        if let Some(name) = entity_name {
            let escaped = sql_escape(name);
            entity_conditions
                .push(format!("(source_name = '{escaped}' OR target_name = '{escaped}')"));
        }
        let entity_filter = format!("WHERE {}", entity_conditions.join(" AND "));

        let batches = self
            .sql(&format!(
                r#"
WITH bounds AS (
    SELECT
        CAST(MIN(FLOOR(combat_time_secs / {bucket_secs})) as BIGINT) as min_bucket,
        CAST(MAX(FLOOR(combat_time_secs / {bucket_secs})) as BIGINT) as max_bucket
    FROM events
    {tr_filter}
),
time_series AS (
    SELECT
      unnest(generate_series(bounds.min_bucket, bounds.max_bucket, 1)) * {bucket_secs} * 1000 AS bucket_start_ms
    FROM bounds
),
hp_events AS (
    SELECT
        CAST(FLOOR(combat_time_secs / {bucket_secs}) * {bucket_secs} * 1000 AS BIGINT) as bucket_start_ms,
        line_number,
        CASE
            WHEN source_name = COALESCE({entity_param}, source_name) AND source_max_hp > 0
                THEN CAST(source_hp AS DOUBLE) * 100.0 / CAST(source_max_hp AS DOUBLE)
            WHEN target_name = COALESCE({entity_param}, target_name) AND target_max_hp > 0
                THEN CAST(target_hp AS DOUBLE) * 100.0 / CAST(target_max_hp AS DOUBLE)
            ELSE NULL
        END as hp_pct,
        CASE
            WHEN source_name = COALESCE({entity_param}, source_name) AND source_max_hp > 0
                THEN source_hp
            WHEN target_name = COALESCE({entity_param}, target_name) AND target_max_hp > 0
                THEN target_hp
            ELSE NULL
        END as current_hp,
        CASE
            WHEN source_name = COALESCE({entity_param}, source_name) AND source_max_hp > 0
                THEN source_max_hp
            WHEN target_name = COALESCE({entity_param}, target_name) AND target_max_hp > 0
                THEN target_max_hp
            ELSE NULL
        END as max_hp
    FROM events
    {entity_filter}
),
ranked AS (
    SELECT
        bucket_start_ms,
        hp_pct,
        current_hp,
        max_hp,
        ROW_NUMBER() OVER (PARTITION BY bucket_start_ms ORDER BY line_number DESC) as rn
    FROM hp_events
    WHERE hp_pct IS NOT NULL
)
SELECT
    time_series.bucket_start_ms,
    ranked.hp_pct,
    ranked.current_hp,
    ranked.max_hp
FROM time_series
LEFT JOIN ranked ON time_series.bucket_start_ms = ranked.bucket_start_ms AND ranked.rn = 1
ORDER BY time_series.bucket_start_ms
            "#,
                entity_param = entity_name
                    .map(|n| format!("'{}'", sql_escape(n)))
                    .unwrap_or_else(|| "NULL".to_string()),
            ))
            .await?;

        // Forward-fill: carry last known HP values through empty buckets
        let mut results = Vec::new();
        let mut last_pct = 100.0f64;
        let mut last_hp = 0i64;
        let mut last_max = 0i64;

        for batch in &batches {
            let buckets = col_i64(batch, 0)?;
            let pct_col = batch.column(1);
            let hp_col = batch.column(2);
            let max_col = batch.column(3);

            for i in 0..batch.num_rows() {
                if !pct_col.is_null(i) {
                    // Extract values — columns may be f64/i32/i64 depending on cast
                    if let Some(a) = pct_col.as_any().downcast_ref::<Float64Array>() {
                        last_pct = a.value(i);
                    }
                    if let Some(a) = hp_col.as_any().downcast_ref::<Int32Array>() {
                        last_hp = a.value(i) as i64;
                    } else if let Some(a) = hp_col.as_any().downcast_ref::<Int64Array>() {
                        last_hp = a.value(i);
                    }
                    if let Some(a) = max_col.as_any().downcast_ref::<Int32Array>() {
                        last_max = a.value(i) as i64;
                    } else if let Some(a) = max_col.as_any().downcast_ref::<Int64Array>() {
                        last_max = a.value(i);
                    }
                }
                results.push(HpPoint {
                    bucket_start_ms: buckets[i],
                    hp_pct: last_pct,
                    current_hp: last_hp,
                    max_hp: last_max,
                });
            }
        }
        Ok(results)
    }
}
