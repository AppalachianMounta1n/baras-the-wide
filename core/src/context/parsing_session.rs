use crate::CombatEvent;
use crate::context::AppConfig;
use crate::events::{EventProcessor, GameSignal, SignalHandler};
use crate::session::SessionCache;
use chrono::NaiveDateTime;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct ParsingSession {
    pub current_byte: Option<u64>,
    pub active_file: Option<PathBuf>,
    pub game_session_date: Option<NaiveDateTime>,
    pub session_cache: Option<SessionCache>,
    processor: EventProcessor,
    signal_handlers: Vec<Box<dyn SignalHandler + Send + Sync>>,
}

impl ParsingSession {
    pub fn new(path: PathBuf) -> Self {
        let date_stamp = parse_log_timestamp(&path);
        Self {
            current_byte: None,
            active_file: Some(path),
            game_session_date: date_stamp,
            session_cache: Some(SessionCache::new()),
            processor: EventProcessor::new(),
            signal_handlers: Vec::new(),
        }
    }

    /// Register a signal handler to receive game signals
    pub fn add_signal_handler(&mut self, handler: Box<dyn SignalHandler + Send + Sync>) {
        self.signal_handlers.push(handler);
    }

    /// Process a single event through the processor and dispatch signals
    pub fn process_event(&mut self, event: CombatEvent) {
        if let Some(cache) = &mut self.session_cache {
            let signals = self.processor.process_event(event, cache);
            self.dispatch_signals(&signals);
        }
    }

    /// Process multiple events
    pub fn process_events(&mut self, events: Vec<CombatEvent>) {
        let mut all_signals = Vec::new();

        if let Some(cache) = &mut self.session_cache {
            for event in events {
                let signals = self.processor.process_event(event, cache);
                all_signals.extend(signals);
            }
        }

        self.dispatch_signals(&all_signals);
    }

    fn dispatch_signals(&mut self, signals: &[GameSignal]) {
        for handler in &mut self.signal_handlers {
            handler.handle_signals(signals);
        }
    }
}

fn parse_log_timestamp(path: &Path) -> Option<NaiveDateTime> {
    let stem = path.file_stem()?.to_str()?.trim_start_matches("combat_");
    NaiveDateTime::parse_from_str(stem, "%Y-%m-%d_%H_%M_%S_%f").ok()
}

/// Resolve a log file path, joining with log_directory if relative.
pub fn resolve_log_path(config: &AppConfig, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        Path::new(&config.log_directory).join(path)
    }
}
