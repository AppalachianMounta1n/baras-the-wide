pub mod context;
pub mod directory_watcher;
pub mod events;
pub mod file_handler;
pub mod handlers;
pub mod log;
pub mod session;
pub mod swtor_ids;
pub mod encounter;

// Legacy module - to be removed after migration
// pub mod encounter;
// pub mod session_cache;

// Re-exports for convenience
pub use events::{EventProcessor, GameSignal, SignalHandler};
pub use session::SessionCache;
pub use swtor_ids::*;
pub use log::*;
