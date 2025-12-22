pub mod context;
pub mod directory_watcher;
pub mod encounter;
pub mod events;
pub mod file_handler;
pub mod handlers;
pub mod log;
pub mod session;
pub mod swtor_ids;
pub mod tracking;

// Re-exports for convenience
pub use events::{EventProcessor, GameSignal, SignalHandler};
pub use handlers::{EffectTracker, NewTargetInfo};
pub use session::SessionCache;
pub use swtor_ids::*;
pub use tracking::{ActiveEffect, DefinitionSet, EffectDefinition, load_definitions};
pub use log::*;
