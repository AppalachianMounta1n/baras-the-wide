use chrono::NaiveDateTime;

#[derive(Debug, Clone, Default)]
pub struct AreaInfo {
    pub area_name: String,
    pub area_id: i64,
    pub entered_at: Option<NaiveDateTime>,
}
