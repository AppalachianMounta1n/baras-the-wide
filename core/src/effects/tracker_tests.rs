//! Tests for effect tracker
//!
//! Verifies instant alert behavior and OnApply alert fixes.

use chrono::Local;

use super::definition::EffectDefinition;
use super::tracker::{DefinitionSet, EffectTracker};
use crate::combat_log::EntityType;
use crate::context::empty_istr;
use crate::dsl::{AudioConfig, EffectSelector, EntityFilter, Trigger};
use crate::signal_processor::{GameSignal, SignalHandler};
use baras_types::AlertTrigger;

fn now() -> chrono::NaiveDateTime {
    Local::now().naive_local()
}

/// Create a minimal effect definition for testing
fn make_effect(
    id: &str,
    name: &str,
    trigger: Trigger,
    duration_secs: Option<f32>,
) -> EffectDefinition {
    EffectDefinition {
        id: id.to_string(),
        name: name.to_string(),
        display_text: None,
        enabled: true,
        trigger,
        ignore_effect_removed: false,
        refresh_abilities: vec![],
        is_refreshed_on_modify: false,
        default_charges: None,
        duration_secs,
        is_affected_by_alacrity: false,
        cooldown_ready_secs: 0.0,
        color: None,
        show_at_secs: 0.0,
        display_target: super::definition::DisplayTarget::None,
        icon_ability_id: None,
        show_icon: true,
        display_source: false,
        disciplines: vec![],
        persist_past_death: false,
        track_outside_combat: true,
        on_apply_trigger_timer: None,
        on_expire_trigger_timer: None,
        is_alert: false,
        alert_text: None,
        alert_on: AlertTrigger::None,
        audio: AudioConfig::default(),
    }
}

fn make_tracker(defs: Vec<EffectDefinition>) -> EffectTracker {
    let mut def_set = DefinitionSet::new();
    def_set.add_definitions(defs, true);
    EffectTracker::new(def_set)
}

fn effect_applied_signal(effect_id: i64, timestamp: chrono::NaiveDateTime) -> GameSignal {
    GameSignal::EffectApplied {
        effect_id,
        effect_name: empty_istr(),
        action_id: 0,
        action_name: empty_istr(),
        source_id: 1,
        source_name: empty_istr(),
        source_entity_type: EntityType::Player,
        source_npc_id: 0,
        target_id: 2,
        target_name: empty_istr(),
        target_entity_type: EntityType::Player,
        target_npc_id: 0,
        timestamp,
        charges: None,
    }
}

fn ability_activated_signal(ability_id: i64, timestamp: chrono::NaiveDateTime) -> GameSignal {
    GameSignal::AbilityActivated {
        ability_id,
        ability_name: empty_istr(),
        source_id: 1,
        source_entity_type: EntityType::Player,
        source_name: empty_istr(),
        source_npc_id: 0,
        target_id: 2,
        target_name: empty_istr(),
        target_entity_type: EntityType::Player,
        target_npc_id: 0,
        timestamp,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Instant Alert: EffectApplied trigger
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_instant_alert_effect_applied_fires_alert_no_active_effect() {
    let mut def = make_effect(
        "test_alert",
        "Test Alert",
        Trigger::EffectApplied {
            effects: vec![EffectSelector::Id(12345)],
            source: EntityFilter::Any,
            target: EntityFilter::Any,
        },
        None,
    );
    def.is_alert = true;
    def.alert_text = Some("Danger!".to_string());

    let mut tracker = make_tracker(vec![def]);
    let ts = now();

    tracker.handle_signal(&effect_applied_signal(12345, ts), None);

    // Should fire alert
    let alerts = tracker.take_fired_alerts();
    assert_eq!(alerts.len(), 1, "Expected 1 fired alert");
    assert_eq!(alerts[0].text, "Danger!");
    assert!(alerts[0].alert_text_enabled);

    // Should NOT create an active effect
    let active_count = tracker.active_effects().count();
    assert_eq!(
        active_count, 0,
        "Instant alert should not create active effect"
    );
}

#[test]
fn test_instant_alert_no_text_when_alert_text_is_none() {
    let mut def = make_effect(
        "test_alert",
        "My Alert Name",
        Trigger::EffectApplied {
            effects: vec![EffectSelector::Id(12345)],
            source: EntityFilter::Any,
            target: EntityFilter::Any,
        },
        None,
    );
    def.is_alert = true;
    def.audio.enabled = true;
    def.audio.file = Some("beep.mp3".to_string());
    // alert_text is None — text overlay should NOT fire, but audio should

    let mut tracker = make_tracker(vec![def]);
    tracker.handle_signal(&effect_applied_signal(12345, now()), None);

    let alerts = tracker.take_fired_alerts();
    assert_eq!(alerts.len(), 1);
    // Text field is populated (for TTS fallback) but alert_text_enabled is false
    assert_eq!(alerts[0].text, "My Alert Name");
    assert!(
        !alerts[0].alert_text_enabled,
        "No text overlay when alert_text is None"
    );
    // Audio still fires
    assert!(alerts[0].audio_enabled);
    assert_eq!(alerts[0].audio_file.as_deref(), Some("beep.mp3"));
}

#[test]
fn test_instant_alert_carries_audio_config() {
    let mut def = make_effect(
        "test_alert",
        "Test Alert",
        Trigger::EffectApplied {
            effects: vec![EffectSelector::Id(12345)],
            source: EntityFilter::Any,
            target: EntityFilter::Any,
        },
        None,
    );
    def.is_alert = true;
    def.alert_text = Some("Watch out!".to_string());
    def.audio.enabled = true;
    def.audio.file = Some("warning.mp3".to_string());

    let mut tracker = make_tracker(vec![def]);
    tracker.handle_signal(&effect_applied_signal(12345, now()), None);

    let alerts = tracker.take_fired_alerts();
    assert_eq!(alerts.len(), 1);
    assert!(alerts[0].audio_enabled);
    assert_eq!(alerts[0].audio_file.as_deref(), Some("warning.mp3"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Instant Alert: AbilityCast trigger
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_instant_alert_ability_cast_fires_alert_no_active_effect() {
    let mut def = make_effect(
        "test_ability_alert",
        "Ability Alert",
        Trigger::AbilityCast {
            abilities: vec![crate::dsl::AbilitySelector::Id(99999)],
            source: EntityFilter::Any,
            target: EntityFilter::Any,
        },
        None,
    );
    def.is_alert = true;
    def.alert_text = Some("Ability fired!".to_string());

    let mut tracker = make_tracker(vec![def]);
    tracker.handle_signal(&ability_activated_signal(99999, now()), None);

    let alerts = tracker.take_fired_alerts();
    assert_eq!(alerts.len(), 1, "Expected 1 fired alert for ability cast");
    assert_eq!(alerts[0].text, "Ability fired!");

    let active_count = tracker.active_effects().count();
    assert_eq!(
        active_count, 0,
        "Instant alert should not create active effect"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Non-instant (is_alert=false) — regression tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_non_instant_effect_creates_active_effect() {
    let def = make_effect(
        "normal_effect",
        "Normal Effect",
        Trigger::EffectApplied {
            effects: vec![EffectSelector::Id(12345)],
            source: EntityFilter::Any,
            target: EntityFilter::Any,
        },
        Some(15.0),
    );

    let mut tracker = make_tracker(vec![def]);
    tracker.handle_signal(&effect_applied_signal(12345, now()), None);

    // Should create active effect
    let active_count = tracker.active_effects().count();
    assert_eq!(
        active_count, 1,
        "Normal effect should create an active effect"
    );

    // Should NOT fire an alert (alert_on is None)
    let alerts = tracker.take_fired_alerts();
    assert!(alerts.is_empty(), "No alert expected for alert_on=None");
}

// ─────────────────────────────────────────────────────────────────────────────
// OnApply alert fix for AbilityCast triggers
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_on_apply_alert_fires_for_ability_cast_trigger() {
    let mut def = make_effect(
        "proc_with_alert",
        "Proc Alert",
        Trigger::AbilityCast {
            abilities: vec![crate::dsl::AbilitySelector::Id(99999)],
            source: EntityFilter::Any,
            target: EntityFilter::Any,
        },
        Some(10.0),
    );
    def.alert_on = AlertTrigger::OnApply;
    def.alert_text = Some("Proc activated!".to_string());

    let mut tracker = make_tracker(vec![def]);
    tracker.handle_signal(&ability_activated_signal(99999, now()), None);

    // Should create active effect AND fire alert
    let active_count = tracker.active_effects().count();
    assert_eq!(active_count, 1, "Should create active effect");

    let alerts = tracker.take_fired_alerts();
    assert_eq!(
        alerts.len(),
        1,
        "Expected OnApply alert for ability cast trigger"
    );
    assert_eq!(alerts[0].text, "Proc activated!");
}
