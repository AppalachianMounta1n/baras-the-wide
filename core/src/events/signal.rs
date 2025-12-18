use crate::log::EntityType;
use chrono::NaiveDateTime;

/// Signals emitted by the EventProcessor for cross-cutting concerns.
/// These represent "interesting things that happened" at a higher level
/// than raw log events.
#[derive(Debug, Clone)]
pub enum GameSignal {
    // Combat lifecycle
    CombatStarted {
        timestamp: NaiveDateTime,
        encounter_id: u64,
    },
    CombatEnded {
        timestamp: NaiveDateTime,
        encounter_id: u64,
    },

    // Entity state changes
    EntityDeath {
        entity_id: i64,
        entity_type: EntityType,
        timestamp: NaiveDateTime,
    },
    EntityRevived {
        entity_id: i64,
        entity_type: EntityType,
        timestamp: NaiveDateTime,
    },

    // Effect tracking
    EffectApplied {
        effect_id: i64,
        source_id: i64,
        target_id: i64,
        timestamp: NaiveDateTime,
    },
    EffectRemoved {
        effect_id: i64,
        source_id: i64,
        target_id: i64,
        timestamp: NaiveDateTime,
    },

    // Ability activation (for timer triggers)
    AbilityActivated {
        ability_id: i64,
        source_id: i64,
        timestamp: NaiveDateTime,
    },

    // Area transitions
    AreaEntered {
        area_id: i64,
        timestamp: NaiveDateTime,
    },

    // Player initialization
    PlayerInitialized {
        entity_id: i64,
        timestamp: NaiveDateTime,
    },
}
