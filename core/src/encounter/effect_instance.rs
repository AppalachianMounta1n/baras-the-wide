use chrono::NaiveDateTime;

#[derive(Debug, Clone)]
pub struct EffectInstance {
    pub effect_id: i64,
    pub source_id: i64,
    pub target_id: i64,
    pub applied_at: NaiveDateTime,
    pub removed_at: Option<NaiveDateTime>,
    pub is_shield: bool,
    /// Indicates this shield has been fully consumed (absorbed its max and was removed).
    /// Once set, this shield should not receive additional absorption credit.
    pub has_absorbed: bool,
}
