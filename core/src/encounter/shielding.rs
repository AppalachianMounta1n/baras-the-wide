use crate::encounter::CombatEvent;
use chrono::NaiveDateTime;
use super::EffectInstance;
use super::Encounter;

/// Grace period (in ms) after a shield is removed during which it can still
/// receive credit for absorption. Accounts for log timing discrepancies.
pub const SHIELD_GRACE_PERIOD_MS: i64 = 2000;


impl Encounter {
   /// Attributes absorbed damage to the appropriate shield source(s).
    ///
    /// When multiple shields are active on a target, absorption is split:
    /// - Single shield: receives full absorption credit
    /// - Two+ shields: first (oldest) shield receives primary credit,
    ///   second shield receives remainder if applicable
    ///
    /// A shield is considered "active" if:
    /// - It was applied before the damage event
    /// - It hasn't been marked as consumed (`has_absorbed`)
    /// - It's either still active OR was removed within the grace period
    pub fn attribute_shield_absorption(&mut self, event: &CombatEvent) {
        let Some(effects) = self.effects.get_mut(&event.target_entity.log_id) else {
            return;
        };

        let mut active_shield_indices: Vec<usize> = effects
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.is_shield
                    && !e.has_absorbed
                    && e.applied_at < event.timestamp
                    && is_shield_active_at(e, event.timestamp)
            })
            .map(|(i, _)| i)
            .collect();

        active_shield_indices.sort_by_key(|&i| effects[i].applied_at);

        if active_shield_indices.is_empty() {
            return;
        }

        let absorbed = event.details.dmg_absorbed as i64;
        let total_dmg = event.details.dmg_amount as i64;

        match active_shield_indices.len() {
            1 => {
                // Single shield: gets all absorption credit
                let shield = &mut effects[active_shield_indices[0]];
                let source_id = shield.source_id;

                // Mark as consumed if the shield was removed (depleted) and damage got through
                if shield.removed_at.is_some() && event.details.dmg_effective > 0 {
                    shield.has_absorbed = true;
                }

                let acc = self.accumulated_data.entry(source_id).or_default();
                acc.shielding_given += absorbed;
            }
            _ => {
                // Multiple shields: split absorption between first two
                let first_idx = active_shield_indices[0];
                let second_idx = active_shield_indices[1];

                let first_source = effects[first_idx].source_id;
                let second_source = effects[second_idx].source_id;

                // Determine how to split the absorption
                // If absorbed == total damage, first shield took it all
                // Otherwise, split: first gets (total - absorbed), second gets absorbed
                let (first_portion, second_portion) = if absorbed >= total_dmg {
                    (absorbed, 0i64)
                } else {
                    // First shield absorbed partial, second absorbed remainder
                    // This matches Orbs' heuristic: first_portion = total - absorbed - mitigated
                    // Since we don't track separate mitigation, use: first gets overflow, second gets logged absorbed
                    let first = total_dmg.saturating_sub(absorbed);
                    let second = absorbed;
                    (first, second)
                };

                // Mark first shield as consumed if it was removed and damage got through
                if effects[first_idx].removed_at.is_some() && event.details.dmg_effective > 0 {
                    effects[first_idx].has_absorbed = true;
                }

                // Credit first shield
                if first_portion > 0 {
                    let acc = self.accumulated_data.entry(first_source).or_default();
                    acc.shielding_given += first_portion;
                }

                // Credit second shield
                if second_portion > 0 {
                    // Mark second as consumed if it was removed and damage got through
                    if effects[second_idx].removed_at.is_some() && event.details.dmg_effective > 0 {
                        effects[second_idx].has_absorbed = true;
                    }
                    let acc = self.accumulated_data.entry(second_source).or_default();
                    acc.shielding_given += second_portion;
                }
            }
        }
    }

}

    /// Checks if a shield effect is active at the given timestamp.
    /// A shield is active if:
    /// - It has no removal time (still active), OR
    /// - It was removed but within the grace period after the event
    #[inline]
    fn is_shield_active_at(effect: &EffectInstance, timestamp: NaiveDateTime) -> bool {
        match effect.removed_at {
            None => true,
            Some(removed) => {
                // Shield is active if removed_at >= timestamp (still active at event time)
                // OR if removed within grace period after the event
                removed >= timestamp
                    || removed
                        .signed_duration_since(timestamp)
                        .num_milliseconds()
                        .abs()
                        <= SHIELD_GRACE_PERIOD_MS
            }
        }
    }
