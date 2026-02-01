//! Combat state machine for encounter lifecycle management.
//!
//! The combat state machine tracks the lifecycle of encounters:
//! - NotStarted: Waiting for combat to begin
//! - InCombat: Active combat, accumulating data
//! - PostCombat: Combat ended, grace period for trailing damage
//!
//! This module handles transitions between states and emits CombatStarted/CombatEnded signals.

use chrono::NaiveDateTime;

use crate::combat_log::CombatEvent;
use crate::encounter::EncounterState;
use crate::game_data::{effect_id, effect_type_id};
use crate::state::SessionCache;

use super::GameSignal;

/// Timeout in seconds before combat ends due to inactivity.
pub const COMBAT_TIMEOUT_SECONDS: i64 = 60;

/// Grace period for boss encounters before finalizing combat end (seconds).
/// Allows merging fake combat splits (e.g., loot chest "enemies", Kephess SM walker).
const BOSS_COMBAT_EXIT_GRACE_SECS: i64 = 3;

/// Grace period for non-boss encounters before finalizing combat end (seconds).
const TRASH_COMBAT_EXIT_GRACE_SECS: i64 = 1;

/// Check if we're within the grace window after a combat exit.
/// Returns the grace duration if within window, None otherwise.
fn within_grace_window(cache: &SessionCache, timestamp: NaiveDateTime) -> bool {
    let Some(exit_time) = cache.last_combat_exit_time else {
        return false;
    };

    let grace_secs = if cache
        .current_encounter()
        .map_or(false, |e| e.active_boss_idx().is_some())
    {
        BOSS_COMBAT_EXIT_GRACE_SECS
    } else {
        TRASH_COMBAT_EXIT_GRACE_SECS
    };

    timestamp.signed_duration_since(exit_time).num_seconds() <= grace_secs
}

/// Advance the combat state machine and emit CombatStarted/CombatEnded signals.
/// Returns (signals, was_accumulated) where was_accumulated indicates whether
/// the event was added to accumulated_data (for parquet write filtering).
pub fn advance_combat_state(event: &CombatEvent, cache: &mut SessionCache) -> (Vec<GameSignal>, bool) {
    // Track effect applications/removals for shield absorption
    track_encounter_effects(event, cache);

    let effect_id = event.effect.effect_id;
    let effect_type_id = event.effect.type_id;
    let timestamp = event.timestamp;

    let current_state = cache
        .current_encounter()
        .map(|e| e.state.clone())
        .unwrap_or_default();

    match current_state {
        EncounterState::NotStarted => handle_not_started(event, cache, effect_id, timestamp),
        EncounterState::InCombat => {
            handle_in_combat(event, cache, effect_id, effect_type_id, timestamp)
        }
        EncounterState::PostCombat { .. } => handle_post_combat(event, cache, effect_id, timestamp),
    }
}

/// Track effect applications/removals in the encounter for shield absorption calculation.
fn track_encounter_effects(event: &CombatEvent, cache: &mut SessionCache) {
    use crate::combat_log::EntityType;

    let Some(enc) = cache.current_encounter_mut() else {
        return;
    };

    match event.effect.type_id {
        effect_type_id::APPLYEFFECT if event.target_entity.entity_type != EntityType::Empty => {
            enc.apply_effect(event);
        }
        effect_type_id::REMOVEEFFECT if event.source_entity.entity_type != EntityType::Empty => {
            enc.remove_effect(event);
        }
        _ => {}
    }
}

fn handle_not_started(
    event: &CombatEvent,
    cache: &mut SessionCache,
    effect_id: i64,
    timestamp: NaiveDateTime,
) -> (Vec<GameSignal>, bool) {
    let mut signals = Vec::new();
    let mut was_accumulated = false;

    if effect_id == effect_id::ENTERCOMBAT {
        if let Some(enc) = cache.current_encounter_mut() {
            enc.state = EncounterState::InCombat;
            enc.enter_combat_time = Some(timestamp);
            enc.track_event_entities(event);
            enc.accumulate_data(event);
            enc.track_event_line(event.line_number);
            was_accumulated = true;

            signals.push(GameSignal::CombatStarted {
                timestamp,
                encounter_id: enc.id,
            });
        }
    } else if effect_id != effect_id::DAMAGE {
        // Buffer non-damage events for the upcoming encounter (skip pre-combat damage)
        if let Some(enc) = cache.current_encounter_mut() {
            enc.accumulate_data(event);
            enc.track_event_line(event.line_number);
            was_accumulated = true;
        }
    }

    (signals, was_accumulated)
}

fn handle_in_combat(
    event: &CombatEvent,
    cache: &mut SessionCache,
    effect_id: i64,
    effect_type_id: i64,
    timestamp: NaiveDateTime,
) -> (Vec<GameSignal>, bool) {
    let mut signals = Vec::new();
    let mut was_accumulated = false;

    // Check for combat timeout
    if let Some(enc) = cache.current_encounter()
        && let Some(last_activity) = enc.last_combat_activity_time
    {
        let elapsed = timestamp.signed_duration_since(last_activity).num_seconds();
        if elapsed >= COMBAT_TIMEOUT_SECONDS {
            let encounter_id = enc.id;
            // End combat at last_activity_time
            if let Some(enc) = cache.current_encounter_mut() {
                enc.exit_combat_time = Some(last_activity);
                enc.state = EncounterState::PostCombat {
                    exit_time: last_activity,
                };
                let duration = enc.duration_seconds().unwrap_or(0) as f32;
                enc.challenge_tracker.finalize(last_activity, duration);
            }

            signals.push(GameSignal::CombatEnded {
                timestamp: last_activity,
                encounter_id,
            });

            cache.push_new_encounter();
            // Re-process this event in the new encounter's state machine
            let (new_signals, new_accumulated) = advance_combat_state(event, cache);
            signals.extend(new_signals);
            return (signals, new_accumulated);
        }
    }

    let all_players_dead = cache
        .current_encounter()
        .map(|e| e.all_players_dead)
        .unwrap_or(false);

    // Check if local player received the post-death revive immortality buff
    // This means they clicked revive and are now out of combat with a grace period
    let local_player_revived = effect_type_id == effect_type_id::APPLYEFFECT
        && effect_id == effect_id::RECENTLY_REVIVED
        && cache.player_initialized
        && event.source_entity.log_id == cache.player.id;

    // Check if all kill targets are dead (boss encounter victory condition)
    // We check all NPC INSTANCES that match kill target class_ids
    let all_kill_targets_dead = cache.current_encounter().map_or(false, |enc| {
        let Some(def_idx) = enc.active_boss_idx() else {
            return false;
        };

        // Collect all kill target class IDs from the boss definition
        let kill_target_class_ids: std::collections::HashSet<i64> = enc.boss_definitions()[def_idx]
            .kill_targets()
            .flat_map(|e| e.ids.iter().copied())
            .collect();

        if kill_target_class_ids.is_empty() {
            return false;
        }

        // Find all NPC instances that are kill targets (by class_id)
        let kill_target_instances: Vec<_> = enc
            .npcs
            .values()
            .filter(|npc| kill_target_class_ids.contains(&npc.class_id))
            .collect();

        // Must have seen at least one kill target instance
        if kill_target_instances.is_empty() {
            return false;
        }

        // All seen kill target instances must be dead
        kill_target_instances.iter().all(|npc| npc.is_dead)
    });

    // Check if this is a boss encounter (has boss definitions loaded OR boss NPCs detected)
    // For boss encounters, we don't want to end on local_player_revived because SWTOR
    // log buffering can cause RECENTLY_REVIVED to arrive before other players' DEATH events
    let is_boss_encounter = cache.current_encounter().map_or(false, |enc| {
        // Has boss definitions loaded for this area
        !enc.boss_definitions().is_empty()
        // OR has detected any boss NPCs in the encounter
        || enc.npcs.values().any(|npc| npc.is_boss)
    });

    // Only end non-boss encounters on local_player_revived
    // For boss fights, rely on all_players_dead or all_kill_targets_dead
    let should_end_on_local_revive = local_player_revived && !is_boss_encounter;

    // Check if this is a victory-trigger encounter that hasn't triggered yet
    // If so, ignore ExitCombat events until the victory trigger fires
    let should_ignore_exit_combat = cache.current_encounter().map_or(false, |enc| {
        // Only check active boss - don't use fallback to avoid false positives
        if let Some(idx) = enc.active_boss_idx() {
            enc.boss_definitions()[idx].has_victory_trigger && !enc.victory_triggered
        } else {
            false
        }
    });

    if effect_id == effect_id::ENTERCOMBAT {
        // Ignore - local player re-entering combat mid-fight (e.g., after battle rez)
        // ENTERCOMBAT only fires for local player, so this is always a rejoin scenario
    } else if should_ignore_exit_combat {
        // For victory-trigger encounters, ignore all exit conditions except all_players_dead (wipe)
        if !all_players_dead {
            // Ignore all other exit conditions (ExitCombat, kill targets, local revive, etc.)
            if let Some(enc) = cache.current_encounter_mut() {
                enc.track_event_entities(event);
                enc.accumulate_data(event);
                enc.track_event_line(event.line_number);
                was_accumulated = true;
            }
            return (signals, was_accumulated); // Don't process further
        }
        // If all_players_dead, fall through to normal exit handling (wipe)
    }
    
    if all_players_dead
        || effect_id == effect_id::EXITCOMBAT
        || all_kill_targets_dead
        || should_end_on_local_revive
    {
        // Check if we're within a grace window from a previous exit
        // If so, this is the "real" exit after a fake enter (holocron case)
        if within_grace_window(cache, timestamp) {
            let exit_time = cache.last_combat_exit_time.unwrap();
            let encounter_id = cache.current_encounter().map(|e| e.id).unwrap_or(0);

            if let Some(enc) = cache.current_encounter_mut() {
                enc.exit_combat_time = Some(exit_time);
                enc.state = EncounterState::PostCombat {
                    exit_time,
                };
                let duration = enc.duration_seconds().unwrap_or(0) as f32;
                enc.challenge_tracker.finalize(exit_time, duration);
            }

            tracing::info!(
                "[COMBAT-STATE] Ending encounter {} at {} (within grace window)",
                encounter_id,
                exit_time
            );
            
            signals.push(GameSignal::CombatEnded {
                timestamp: exit_time,
                encounter_id,
            });

            cache.last_combat_exit_time = None;
            cache.push_new_encounter();
        } else {
            // Start grace window - don't emit CombatEnded yet
            cache.last_combat_exit_time = Some(timestamp);

            if let Some(enc) = cache.current_encounter_mut() {
                enc.exit_combat_time = Some(timestamp);
                enc.state = EncounterState::PostCombat {
                    exit_time: timestamp,
                };
                let duration = enc.duration_seconds().unwrap_or(0) as f32;
                enc.challenge_tracker.finalize(timestamp, duration);
            }
            // Note: Don't emit CombatEnded or push_new_encounter yet
        }
    } else if effect_type_id == effect_type_id::AREAENTERED {
        let encounter_id = cache.current_encounter().map(|e| e.id).unwrap_or(0);
        if let Some(enc) = cache.current_encounter_mut() {
            enc.exit_combat_time = Some(timestamp);
            enc.state = EncounterState::PostCombat {
                exit_time: timestamp,
            };
            let duration = enc.duration_seconds().unwrap_or(0) as f32;
            enc.challenge_tracker.finalize(timestamp, duration);
        }

        signals.push(GameSignal::CombatEnded {
            timestamp,
            encounter_id,
        });

        cache.push_new_encounter();
    } else {
        // Normal combat event
        if let Some(enc) = cache.current_encounter_mut() {
            enc.track_event_entities(event);
            enc.accumulate_data(event);
            enc.track_event_line(event.line_number);
            was_accumulated = true;
            if effect_id == effect_id::DAMAGE || effect_id == effect_id::HEAL {
                enc.last_combat_activity_time = Some(timestamp);
            }
        }
    }

    (signals, was_accumulated)
}

fn handle_post_combat(
    event: &CombatEvent,
    cache: &mut SessionCache,
    effect_id: i64,
    timestamp: NaiveDateTime,
) -> (Vec<GameSignal>, bool) {
    let mut signals = Vec::new();
    let mut was_accumulated = false;

    // During grace window, only respond to ENTERCOMBAT (to restore combat)
    // All other events are buffered/ignored until grace expires
    let in_grace_window = within_grace_window(cache, timestamp);

    if effect_id == effect_id::ENTERCOMBAT {
        if in_grace_window {
            // Restore encounter to InCombat - this "corrects" the fake exit
            if let Some(enc) = cache.current_encounter_mut() {
                enc.state = EncounterState::InCombat;
                enc.exit_combat_time = None;
                // Track line number - grace period events are part of this encounter
                enc.track_event_line(event.line_number);
            }
            // Keep last_combat_exit_time set - we'll use it if another exit comes quickly
            // Don't emit any signals - combat "continues"
            // Note: was_accumulated remains false - grace window events not accumulated
        } else {
            // Outside grace window - finalize previous encounter and start new
            finalize_pending_combat_exit(cache, &mut signals);

            let new_encounter_id = cache.push_new_encounter();
            if let Some(enc) = cache.current_encounter_mut() {
                enc.state = EncounterState::InCombat;
                enc.enter_combat_time = Some(timestamp);
                enc.accumulate_data(event);
                enc.track_event_line(event.line_number);
                was_accumulated = true;
            }

            signals.push(GameSignal::CombatStarted {
                timestamp,
                encounter_id: new_encounter_id,
            });
        }
    } else if in_grace_window {
        // During grace window, events are not accumulated but we still track their line numbers
        // for per-encounter Parsely uploads (grace period events belong to this encounter)
        if let Some(enc) = cache.current_encounter_mut() {
            enc.track_event_line(event.line_number);
        }
        // was_accumulated remains false
    } else if effect_id == effect_id::DAMAGE {
        // Discard post-combat damage - start fresh encounter
        finalize_pending_combat_exit(cache, &mut signals);
        cache.push_new_encounter();
        // was_accumulated remains false - damage discarded
    } else {
        // Non-damage event - goes to next encounter
        finalize_pending_combat_exit(cache, &mut signals);
        cache.push_new_encounter();
        if let Some(enc) = cache.current_encounter_mut() {
            enc.accumulate_data(event);
            enc.track_event_line(event.line_number);
            was_accumulated = true;
        }
    }

    (signals, was_accumulated)
}

/// Finalize any pending combat exit (emit CombatEnded if grace window was active).
fn finalize_pending_combat_exit(cache: &mut SessionCache, signals: &mut Vec<GameSignal>) {
    if let Some(exit_time) = cache.last_combat_exit_time.take() {
        let encounter_id = cache.current_encounter().map(|e| e.id).unwrap_or(0);
        signals.push(GameSignal::CombatEnded {
            timestamp: exit_time,
            encounter_id,
        });
    }
}

/// Tick the combat state machine using wall-clock time.
///
/// This provides a fallback timeout when the event stream stops (e.g., player dies
/// and revives but no new combat events arrive). Called periodically from the tail loop.
///
/// Returns CombatEnded signal if combat times out due to inactivity.
/// Also handles grace window expiration for combat exit.
pub fn tick_combat_state(cache: &mut SessionCache) -> Vec<GameSignal> {
    let mut signals = Vec::new();
    let now = chrono::Local::now().naive_local();

    let current_state = cache
        .current_encounter()
        .map(|e| e.state.clone())
        .unwrap_or_default();

    // Check for grace window expiration
    if let Some(exit_time) = cache.last_combat_exit_time {
        let grace_secs = if cache
            .current_encounter()
            .map_or(false, |e| e.active_boss_idx().is_some())
        {
            BOSS_COMBAT_EXIT_GRACE_SECS
        } else {
            TRASH_COMBAT_EXIT_GRACE_SECS
        };

        let elapsed = now.signed_duration_since(exit_time).num_seconds();
        if elapsed > grace_secs {
            match current_state {
                EncounterState::PostCombat { .. } => {
                    // Grace expired while in PostCombat - finalize the encounter
                    let encounter_id = cache.current_encounter().map(|e| e.id).unwrap_or(0);
            signals.push(GameSignal::CombatEnded {
                timestamp: exit_time,
                encounter_id,
            });

            cache.last_combat_exit_time = None;
            cache.push_new_encounter();
                }
                EncounterState::InCombat => {
                    // Grace expired while back in InCombat - Kephess case
                    // The fake exit was corrected, just clear the grace window
                    cache.last_combat_exit_time = None;
                }
                _ => {
                    cache.last_combat_exit_time = None;
                }
            }
            return signals;
        }
    }

    // Only check combat timeout during active combat
    if !matches!(current_state, EncounterState::InCombat) {
        return signals;
    }

    // Check wall-clock timeout
    if let Some(enc) = cache.current_encounter()
        && let Some(last_activity) = enc.last_combat_activity_time
    {
        let elapsed = now.signed_duration_since(last_activity).num_seconds();
        if elapsed >= COMBAT_TIMEOUT_SECONDS {
            let encounter_id = enc.id;

            // End combat at last_activity_time (same as event-driven timeout)
            if let Some(enc) = cache.current_encounter_mut() {
                enc.exit_combat_time = Some(last_activity);
                enc.state = EncounterState::PostCombat {
                    exit_time: last_activity,
                };
                let duration = enc.duration_seconds().unwrap_or(0) as f32;
                enc.challenge_tracker.finalize(last_activity, duration);
            }

            cache.last_combat_exit_time = None;
            cache.push_new_encounter();

            return vec![GameSignal::CombatEnded {
                timestamp: last_activity,
                encounter_id,
            }];
        }
    }

    signals
}
