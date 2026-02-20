//! Counter increment and trigger checking logic.
//!
//! Counters track occurrences during boss encounters (e.g., add spawns, ability casts).
//! This module handles detecting when counters should increment based on game events.
//!
//! Trigger matching delegates to the unified `Trigger::matches_*()` methods in
//! `dsl/triggers/mod.rs` to ensure consistent behavior across timers, phases, and counters.

use crate::combat_log::{CombatEvent, EntityType};
use crate::dsl::{EntityDefinition, EntityFilterMatching, Trigger};
use crate::game_data::{effect_id, effect_type_id};
use crate::state::SessionCache;

use super::GameSignal;

/// Check for counter increments/decrements based on events and emit CounterChanged signals.
pub fn check_counter_increments(
    event: &CombatEvent,
    cache: &mut SessionCache,
    current_signals: &[GameSignal],
) -> Vec<GameSignal> {
    // Clone the Arc (cheap) to hold definitions while we mutate cache
    let (definitions, def_idx) = {
        let Some(enc) = cache.current_encounter() else {
            return Vec::new();
        };
        let Some(idx) = enc.active_boss_idx() else {
            return Vec::new();
        };
        (enc.boss_definitions_arc(), idx)
    };
    let def = &definitions[def_idx];

    let mut signals = Vec::new();

    for counter in &def.counters {
        // Check increment_on trigger
        if check_counter_trigger(&counter.increment_on, event, current_signals, &def.entities) {
            let Some(enc) = cache.current_encounter_mut() else {
                tracing::error!(
                    "BUG: encounter missing in check_counter_increments (increment_on)"
                );
                continue;
            };
            let (old_value, new_value) = enc.modify_counter(
                &counter.id,
                counter.decrement, // Legacy: use decrement flag for increment_on
                counter.set_value,
            );

            signals.push(GameSignal::CounterChanged {
                counter_id: counter.id.clone(),
                old_value,
                new_value,
                timestamp: event.timestamp,
            });
        }

        // Check decrement_on trigger (always decrements)
        if let Some(ref decrement_trigger) = counter.decrement_on
            && check_counter_trigger(decrement_trigger, event, current_signals, &def.entities)
        {
            let Some(enc) = cache.current_encounter_mut() else {
                tracing::error!(
                    "BUG: encounter missing in check_counter_increments (decrement_on)"
                );
                continue;
            };
            let (old_value, new_value) = enc.modify_counter(
                &counter.id,
                true, // Always decrement
                None, // Never set_value for decrement_on
            );

            signals.push(GameSignal::CounterChanged {
                counter_id: counter.id.clone(),
                old_value,
                new_value,
                timestamp: event.timestamp,
            });
        }

        // Check reset_on trigger (resets to initial_value)
        if check_counter_trigger(&counter.reset_on, event, current_signals, &def.entities) {
            let Some(enc) = cache.current_encounter_mut() else {
                tracing::error!("BUG: encounter missing in check_counter_increments (reset_on)");
                continue;
            };
            let old_value = enc.get_counter(&counter.id);
            let new_value = counter.initial_value;

            // Only emit signal if value actually changes
            if old_value != new_value {
                enc.set_counter(&counter.id, new_value);
                signals.push(GameSignal::CounterChanged {
                    counter_id: counter.id.clone(),
                    old_value,
                    new_value,
                    timestamp: event.timestamp,
                });
            }
        }
    }

    signals
}

/// Check for counter changes triggered by timer events (expires/starts).
/// Called after TimerManager processes signals to handle timer→counter triggers.
pub fn check_counter_timer_triggers(
    expired_timer_ids: &[String],
    started_timer_ids: &[String],
    cache: &mut SessionCache,
    timestamp: chrono::NaiveDateTime,
) -> Vec<GameSignal> {
    if expired_timer_ids.is_empty() && started_timer_ids.is_empty() {
        return Vec::new();
    }

    // Clone the Arc (cheap) to hold definitions while we mutate cache
    let (definitions, def_idx) = {
        let Some(enc) = cache.current_encounter() else {
            return Vec::new();
        };
        let Some(idx) = enc.active_boss_idx() else {
            return Vec::new();
        };
        (enc.boss_definitions_arc(), idx)
    };
    let def = &definitions[def_idx];

    let mut signals = Vec::new();

    for counter in &def.counters {
        // Check increment_on for timer triggers
        if matches_timer_trigger(&counter.increment_on, expired_timer_ids, started_timer_ids) {
            let Some(enc) = cache.current_encounter_mut() else {
                tracing::error!(
                    "BUG: encounter missing in check_counter_timer_triggers (increment_on)"
                );
                continue;
            };
            let (old_value, new_value) =
                enc.modify_counter(&counter.id, counter.decrement, counter.set_value);
            signals.push(GameSignal::CounterChanged {
                counter_id: counter.id.clone(),
                old_value,
                new_value,
                timestamp,
            });
        }

        // Check decrement_on for timer triggers
        if let Some(ref trigger) = counter.decrement_on {
            if matches_timer_trigger(trigger, expired_timer_ids, started_timer_ids) {
                let Some(enc) = cache.current_encounter_mut() else {
                    tracing::error!(
                        "BUG: encounter missing in check_counter_timer_triggers (decrement_on)"
                    );
                    continue;
                };
                let (old_value, new_value) = enc.modify_counter(
                    &counter.id,
                    true, // Always decrement
                    None,
                );
                signals.push(GameSignal::CounterChanged {
                    counter_id: counter.id.clone(),
                    old_value,
                    new_value,
                    timestamp,
                });
            }
        }

        // Check reset_on for timer triggers
        if matches_timer_trigger(&counter.reset_on, expired_timer_ids, started_timer_ids) {
            let Some(enc) = cache.current_encounter_mut() else {
                tracing::error!(
                    "BUG: encounter missing in check_counter_timer_triggers (reset_on)"
                );
                continue;
            };
            let old_value = enc.get_counter(&counter.id);
            let new_value = counter.initial_value;
            if old_value != new_value {
                enc.set_counter(&counter.id, new_value);
                signals.push(GameSignal::CounterChanged {
                    counter_id: counter.id.clone(),
                    old_value,
                    new_value,
                    timestamp,
                });
            }
        }
    }

    signals
}

/// Check if a trigger matches any expired or started timer IDs.
/// Handles TimerExpires, TimerStarted, and AnyOf wrappers.
fn matches_timer_trigger(
    trigger: &Trigger,
    expired_timer_ids: &[String],
    started_timer_ids: &[String],
) -> bool {
    match trigger {
        Trigger::TimerExpires { timer_id } => expired_timer_ids.contains(timer_id),
        Trigger::TimerStarted { timer_id } => started_timer_ids.contains(timer_id),
        Trigger::AnyOf { conditions } => conditions
            .iter()
            .any(|c| matches_timer_trigger(c, expired_timer_ids, started_timer_ids)),
        _ => false,
    }
}

/// Check if a counter trigger is satisfied by the current event/signals.
///
/// Delegates to unified `Trigger::matches_*()` methods where possible for consistency
/// with timer and phase trigger evaluation.
pub fn check_counter_trigger(
    trigger: &Trigger,
    event: &CombatEvent,
    current_signals: &[GameSignal],
    entities: &[EntityDefinition],
) -> bool {
    // Try event-based triggers first (from CombatEvent)
    if check_event_based_trigger(trigger, event, entities) {
        return true;
    }

    // Then check signal-based triggers (from GameSignal)
    check_signal_based_trigger(trigger, current_signals, entities)
}

/// Check event-based triggers (AbilityCast, EffectApplied, EffectRemoved).
/// These require checking the raw CombatEvent and applying source/target filters.
fn check_event_based_trigger(
    trigger: &Trigger,
    event: &CombatEvent,
    entities: &[EntityDefinition],
) -> bool {
    match trigger {
        Trigger::AbilityCast { .. } => {
            if event.effect.effect_id != effect_id::ABILITYACTIVATE {
                return false;
            }
            let ability_id = event.action.action_id as u64;
            let ability_name = crate::context::resolve(event.action.name);

            // Delegate ID/name matching to unified method
            if !trigger.matches_ability(ability_id, Some(ability_name)) {
                return false;
            }

            // Check source/target filters
            check_event_source_target(trigger, event, entities)
        }

        Trigger::EffectApplied { .. } => {
            if event.effect.type_id != effect_type_id::APPLYEFFECT {
                return false;
            }
            let effect_id = event.effect.effect_id as u64;
            let effect_name = crate::context::resolve(event.effect.effect_name);

            // Delegate ID/name matching to unified method
            if !trigger.matches_effect_applied(effect_id, Some(effect_name)) {
                return false;
            }

            // Check source/target filters
            check_event_source_target(trigger, event, entities)
        }

        Trigger::EffectRemoved { .. } => {
            if event.effect.type_id != effect_type_id::REMOVEEFFECT {
                return false;
            }
            let effect_id = event.effect.effect_id as u64;
            let effect_name = crate::context::resolve(event.effect.effect_name);

            // Delegate ID/name matching to unified method
            if !trigger.matches_effect_removed(effect_id, Some(effect_name)) {
                return false;
            }

            // Check source/target filters
            check_event_source_target(trigger, event, entities)
        }

        Trigger::AnyOf { conditions } => conditions
            .iter()
            .any(|c| check_event_based_trigger(c, event, entities)),

        _ => false,
    }
}

/// Check source/target filters for event-based triggers.
fn check_event_source_target(
    trigger: &Trigger,
    event: &CombatEvent,
    entities: &[EntityDefinition],
) -> bool {
    // Check source filter
    if let Some(source_filter) = trigger.source_filter() {
        if !source_filter.is_any() {
            if source_filter.is_local_player() {
                if event.source_entity.entity_type != EntityType::Player {
                    return false;
                }
            } else {
                let source_name = crate::context::resolve(event.source_entity.name);
                if !source_filter.matches_source_target(
                    entities,
                    event.source_entity.class_id,
                    source_name,
                ) {
                    return false;
                }
            }
        }
    }

    // Check target filter
    if let Some(target_filter) = trigger.target_filter() {
        if !target_filter.is_any() {
            if target_filter.is_local_player() {
                if event.target_entity.entity_type != EntityType::Player {
                    return false;
                }
            } else {
                let target_name = crate::context::resolve(event.target_entity.name);
                if !target_filter.matches_source_target(
                    entities,
                    event.target_entity.class_id,
                    target_name,
                ) {
                    return false;
                }
            }
        }
    }

    true
}

/// Check signal-based triggers (everything derived from GameSignal).
/// Delegates to unified `Trigger::matches_*()` methods where possible.
fn check_signal_based_trigger(
    trigger: &Trigger,
    signals: &[GameSignal],
    entities: &[EntityDefinition],
) -> bool {
    match trigger {
        // Combat state (simple signal checks)
        Trigger::CombatStart => signals
            .iter()
            .any(|s| matches!(s, GameSignal::CombatStarted { .. })),

        Trigger::CombatEnd => signals
            .iter()
            .any(|s| matches!(s, GameSignal::CombatEnded { .. })),

        // HP thresholds - delegate to unified method (includes crossing check)
        Trigger::BossHpBelow { .. } => signals.iter().any(|s| {
            if let GameSignal::BossHpChanged {
                npc_id,
                entity_name,
                old_hp_percent,
                new_hp_percent,
                ..
            } = s
            {
                trigger.matches_boss_hp_below(
                    entities,
                    *npc_id,
                    entity_name,
                    *old_hp_percent,
                    *new_hp_percent,
                )
            } else {
                false
            }
        }),

        // Entity lifecycle - delegate to unified methods
        Trigger::NpcAppears { .. } => signals.iter().any(|s| {
            if let GameSignal::NpcFirstSeen {
                npc_id,
                entity_name,
                ..
            } = s
            {
                trigger.matches_npc_appears(entities, *npc_id, entity_name)
            } else {
                false
            }
        }),

        Trigger::EntityDeath { .. } => signals.iter().any(|s| {
            if let GameSignal::EntityDeath {
                npc_id,
                entity_name,
                ..
            } = s
            {
                trigger.matches_entity_death(entities, *npc_id, entity_name)
            } else {
                false
            }
        }),

        // Phase events - delegate to unified methods
        Trigger::PhaseEntered { .. } => signals.iter().any(|s| {
            if let GameSignal::PhaseChanged { new_phase, .. } = s {
                trigger.matches_phase_entered(new_phase)
            } else {
                false
            }
        }),

        Trigger::PhaseEnded { .. } => signals.iter().any(|s| {
            match s {
                GameSignal::PhaseChanged {
                    old_phase: Some(old),
                    ..
                } => trigger.matches_phase_ended(old),
                GameSignal::PhaseEndTriggered { phase_id, .. } => {
                    trigger.matches_phase_ended(phase_id)
                }
                _ => false,
            }
        }),

        Trigger::AnyPhaseChange => signals
            .iter()
            .any(|s| matches!(s, GameSignal::PhaseChanged { .. })),

        // Counter events - delegate to unified method (includes crossing check)
        Trigger::CounterReaches { .. } => signals.iter().any(|s| {
            if let GameSignal::CounterChanged {
                counter_id,
                old_value,
                new_value,
                ..
            } = s
            {
                trigger.matches_counter_reaches(counter_id, *old_value, *new_value)
            } else {
                false
            }
        }),

        // Damage taken - delegate ID matching, then check source/target filters
        Trigger::DamageTaken { .. } => signals.iter().any(|s| {
            if let GameSignal::DamageTaken {
                ability_id,
                ability_name,
                source_npc_id,
                source_name,
                target_npc_id,
                target_name,
                ..
            } = s
            {
                let ability_name_str = crate::context::resolve(*ability_name);

                // Delegate ID/name matching to unified method
                if !trigger.matches_damage_taken(*ability_id as u64, Some(ability_name_str)) {
                    return false;
                }

                // Check source/target filters
                check_signal_source_target(
                    trigger,
                    entities,
                    *source_npc_id,
                    *source_name,
                    *target_npc_id,
                    *target_name,
                )
            } else {
                false
            }
        }),

        // Healing taken - delegate ID matching, then check source/target filters
        Trigger::HealingTaken { .. } => signals.iter().any(|s| {
            if let GameSignal::HealingDone {
                ability_id,
                ability_name,
                source_npc_id,
                source_name,
                target_npc_id,
                target_name,
                ..
            } = s
            {
                let ability_name_str = crate::context::resolve(*ability_name);

                if !trigger.matches_healing_taken(*ability_id as u64, Some(ability_name_str)) {
                    return false;
                }

                check_signal_source_target(
                    trigger,
                    entities,
                    *source_npc_id,
                    *source_name,
                    *target_npc_id,
                    *target_name,
                )
            } else {
                false
            }
        }),

        // Counter-specific: never trigger
        Trigger::Never => false,

        // Timer triggers handled separately via check_counter_timer_triggers
        Trigger::TimerExpires { .. } | Trigger::TimerStarted { .. } => false,

        // Event-based triggers handled by check_event_based_trigger, not signals
        Trigger::AbilityCast { .. }
        | Trigger::EffectApplied { .. }
        | Trigger::EffectRemoved { .. } => false,

        // Not applicable to counters
        Trigger::TimeElapsed { .. }
        | Trigger::BossHpAbove { .. }
        | Trigger::TargetSet { .. }
        | Trigger::Manual => false,

        // Composition
        Trigger::AnyOf { conditions } => conditions
            .iter()
            .any(|c| check_signal_based_trigger(c, signals, entities)),
    }
}

/// Check source/target filters for signal-based triggers (DamageTaken).
fn check_signal_source_target(
    trigger: &Trigger,
    entities: &[EntityDefinition],
    source_npc_id: i64,
    source_name: crate::context::IStr,
    target_npc_id: i64,
    target_name: crate::context::IStr,
) -> bool {
    if let Some(source_filter) = trigger.source_filter() {
        if !source_filter.is_any() {
            let source_name_str = crate::context::resolve(source_name);
            if !source_filter.matches_source_target(entities, source_npc_id, source_name_str) {
                return false;
            }
        }
    }

    if let Some(target_filter) = trigger.target_filter() {
        if !target_filter.is_any() {
            let target_name_str = crate::context::resolve(target_name);
            if !target_filter.matches_source_target(entities, target_npc_id, target_name_str) {
                return false;
            }
        }
    }

    true
}
