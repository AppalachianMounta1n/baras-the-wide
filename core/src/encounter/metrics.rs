use super::IStr;
use super::EntityType;

#[derive(Debug, Clone, Default)]
pub struct MetricAccumulator {
  pub  damage_dealt: i64,
  pub  damage_dealt_effective: i64,
  pub  damage_received: i64,
  pub  damage_absorbed: i64,
  pub  healing_effective: i64,
  pub  healing_done: i64,
  pub  healing_received: i64,
  pub  hit_count: u32,
  pub  actions: u32,
  pub  shielding_given: i64,
  pub  threat_generated: f64,
}


#[derive(Debug, Clone)]
pub struct EntityMetrics {
    pub entity_id: i64,
    pub name: IStr,
    pub entity_type: EntityType,
    pub total_damage: i64,
    pub dps: i32,
    pub edps: i32,
    pub hps: i32,
    pub ehps: i32,
    pub dtps: i32,
    pub abs: i32,
    pub total_healing: i64,
    pub apm: f32,
    pub tps: i32,
    pub total_threat: i64,
}
