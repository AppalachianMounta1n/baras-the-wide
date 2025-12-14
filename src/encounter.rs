use crate::{CombatEvent, log_ids::effect_id};
use time::Time;

#[derive(Debug, Clone, Default)]
pub struct Encounter {
    pub events: Vec<CombatEvent>,
    pub enter_time: Option<u64>,
    pub exit_time: Option<u64>,
    pub combat_state: CombatState,
    // future: npcs, duration, etc.
}

impl Encounter {
    fn new() -> Encounter {
        Encounter {
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum CombatState {
    #[default]
    Idle,
    InCombat,
    PostCombat {
        exit_time: Time,
    },
}

pub struct EncounterBuilder {
    state: CombatState,
    buffer: Vec<CombatEvent>, // pre-combat non-damage events
    current: Option<Encounter>,
    completed: Vec<Encounter>,
    post_combat_threshold_ms: i128, // 5000
}

impl EncounterBuilder {
    pub fn process(&mut self, event: CombatEvent) {
        match (&self.state, event.effect.effect_id) {
            // Idle: buffer non-damage, start encounter on CombatEnter
            (CombatState::Idle, effect_id::ENTERCOMBAT) => {
                let mut enc = Encounter::new();
                enc.events.append(&mut self.buffer); // drain pre-combat
                enc.events.push(event);
                self.current = Some(enc);
                self.state = CombatState::InCombat;
            }
            (CombatState::Idle, _) if !event.effect.effect_id == effect_id::DAMAGE => {
                self.buffer.push(event);
            }

            // InCombat: collect everything, transition on CombatExit
            (CombatState::InCombat, effect_id::EXITCOMBAT) => {
                let exit_time = event.timestamp;
                self.current.as_mut().unwrap().events.push(event);
                self.state = CombatState::PostCombat { exit_time };
            }
            (CombatState::InCombat, _) => {
                self.current.as_mut().unwrap().events.push(event);
            }

            // PostCombat: collect damage within threshold, finalize on CombatEnter or timeout
            (CombatState::PostCombat { exit_time }, effect_id::ENTERCOMBAT) => {
                println!("{}", exit_time);
                self.finalize_current();
                // Start new encounter
                let mut enc = Encounter::new();
                enc.events.push(event);
                self.current = Some(enc);
                self.state = CombatState::InCombat;
            }
            (CombatState::PostCombat { exit_time }, _)
                if event.effect.effect_id == effect_id::DAMAGE =>
            {
                if event
                    .timestamp
                    .duration_since(*exit_time)
                    .whole_milliseconds()
                    <= self.post_combat_threshold_ms
                {
                    self.current.as_mut().unwrap().events.push(event);
                } else {
                    self.finalize_current();
                    self.state = CombatState::Idle;
                }
            }
            (CombatState::PostCombat { .. }, _) => {
                // Non-damage after exit → buffer for next encounter
                self.finalize_current();
                self.buffer.push(event);
                self.state = CombatState::Idle;
            }

            _ => {}
        }
    }

    fn finalize_current(&mut self) {
        if let Some(enc) = self.current.take() {
            self.completed.push(enc);
        }
    }
}
