use crate::events::{GameSignal, SignalHandler};

/// Tracks active effects/buffs for overlay display.
/// Clears effects on death, updates on apply/remove signals.
#[derive(Debug, Default)]
pub struct EffectTracker {
    // TODO: Add tracked effects
}

impl EffectTracker {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SignalHandler for EffectTracker {
    fn handle_signal(&mut self, signal: &GameSignal) {
        match signal {
            GameSignal::EffectApplied {
                effect_id,
                source_id,
                target_id,
                timestamp,
            } => {
                // TODO: Track effect application
                let _ = (effect_id, source_id, target_id, timestamp);
            }
            GameSignal::EffectRemoved {
                effect_id,
                source_id,
                target_id,
                timestamp,
            } => {
                // TODO: Remove effect from tracking
                let _ = (effect_id, source_id, target_id, timestamp);
            }
            GameSignal::EntityDeath { entity_id, .. } => {
                // TODO: Clear effects for dead entity
                let _ = entity_id;
            }
            GameSignal::CombatEnded { .. } => {
                // TODO: Optionally clear combat-only effects
            }
            _ => {}
        }
    }
}
