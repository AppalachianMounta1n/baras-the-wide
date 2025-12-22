//! Effect and Timer tracking system
//!
//! This module provides:
//! - **Definitions**: Templates that describe what effects/timers to track
//! - **Active instances**: Runtime state of currently active effects/timers
//! - **Config loading**: TOML-based configuration for builtin and custom definitions
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     Definition (TOML config)                     │
//! │  "Track effect ID 814832605462528 as 'Kolto Probe', green, 20s" │
//! └─────────────────────────────────────────────────────────────────┘
//!                              │
//!                    GameSignal::EffectApplied
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                   ActiveEffect (runtime state)                   │
//! │  "Player 'Tank' has Kolto Probe, applied 3s ago, 2 stacks"      │
//! └─────────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//!                     Overlay Renderer
//! ```

mod active_effect;
mod active_timer;
mod config;
mod definitions;

pub use active_effect::{ActiveEffect, EffectKey};
pub use active_timer::{ActiveTimer, TimerKey};
pub use config::{load_definitions, DefinitionSet, ConfigError};
pub use definitions::{
    DefinitionConfig, EffectCategory, EffectDefinition, EntityFilter, TimerDefinition, TimerTrigger,
};
