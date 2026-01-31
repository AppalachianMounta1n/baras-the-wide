//! UI Components
//!
//! This module contains reusable UI components extracted from app.rs
//! to improve code organization and reduce file size.

pub mod ability_icon;
pub mod charts_panel;
pub mod class_icons;
pub mod combat_log;
pub mod data_explorer;
pub mod effect_editor;
pub mod encounter_editor;
pub mod history_panel;
pub mod hotkey_input;
pub mod parsely_upload_modal;
pub mod phase_timeline;
pub mod settings_panel;
pub mod toast;

// Re-export types that were moved to baras-types for sharing
pub use crate::types::{
    CombatLogSessionState, DataExplorerState, EffectsEditorState, EncounterBuilderState,
    MainTab, SortColumn, SortDirection, UiSessionState, ViewMode,
};

pub use data_explorer::DataExplorerPanel;
pub use effect_editor::EffectEditorPanel;
pub use encounter_editor::EncounterEditorPanel;
pub use history_panel::HistoryPanel;
pub use hotkey_input::HotkeyInput;
pub use parsely_upload_modal::{ParselyUploadModal, use_parsely_upload, use_parsely_upload_provider};
pub use settings_panel::SettingsPanel;
pub use toast::{ToastFrame, ToastManager, ToastSeverity, use_toast, use_toast_provider};
