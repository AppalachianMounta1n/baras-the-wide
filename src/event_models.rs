#[derive(Debug, Clone, Default)]
pub struct Action {
    pub name: String,
    pub action_id: i64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum EntityType {
    Player,
    Npc,
    Companion,
    #[default]
    Empty,
    SelfReference,
}

#[derive(Debug, Clone, Default)]
pub struct Timestamp {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millis: u16,
}

#[derive(Debug, Clone, Default)]
pub struct Entity {
    pub name: String,
    pub class_id: i64,
    pub log_id: i64,
    pub entity_type: EntityType,
    pub health: (i32, i32),
}

#[derive(Debug, Clone, Default)]
pub struct CombatEvent {
    pub line_number: usize,
    pub timestamp: Timestamp,
    pub source_entity: Entity,
    pub target_entity: Entity,
    pub action: Action,
    pub effect: Effect,
    pub charges: Option<i64>,
    pub damage: Option<i64>,
    pub effective_damage: Option<i64>,
    pub damage_type_id: Option<String>,
    pub is_critical: Option<bool>,
    pub is_reflected: Option<bool>,
    pub threat: Option<f64>,
    pub reduction_class_id: Option<String>,
    pub damage_reduced: Option<String>,
    pub reduction_type_id: Option<String>,
    pub heal: Option<i64>,
    pub effective_heal: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub enum Effect {
    #[default]
    Empty,

    Standard {
        type_name: String,
        type_id: i64,
        name: String,
        id: i64,
    },

    DisciplineChanged {
        type_id: i64,
        class_name: String,
        class_id: i64,
        discipline_name: String,
        discipline_id: i64,
    },

    AreaEntered {
        type_id: i64,
        area_name: String,
        area_id: i64,
        difficulty: Option<String>,
        difficulty_id: Option<i64>,
    },
}
