//! Notes editing tab
//!
//! Freeform Markdown text editor for encounter notes.
//! Notes are displayed on the Notes overlay during gameplay.

use dioxus::prelude::*;

use crate::api;
use crate::types::BossWithPath;

// ─────────────────────────────────────────────────────────────────────────────
// Notes Tab
// ─────────────────────────────────────────────────────────────────────────────

#[component]
pub fn NotesTab(
    boss_with_path: BossWithPath,
    on_change: EventHandler<Option<String>>,
    on_status: EventHandler<(String, bool)>,
) -> Element {
    // Local state for the textarea
    let mut notes_text = use_signal(|| boss_with_path.boss.notes.clone().unwrap_or_default());
    let mut is_dirty = use_signal(|| false);
    let mut is_saving = use_signal(|| false);

    // Extract context for API calls
    let boss_id = boss_with_path.boss.id.clone();
    let file_path = boss_with_path.file_path.clone();

    // Clone values needed for save handlers
    let boss_id_btn = boss_id.clone();
    let file_path_btn = file_path.clone();
    let boss_id_kbd = boss_id.clone();
    let file_path_kbd = file_path.clone();

    rsx! {
        div { class: "notes-tab",
            // Header with save button
            div { class: "flex items-center justify-between mb-sm",
                div { class: "flex items-center gap-sm",
                    span { class: "text-sm text-secondary", "Encounter Notes" }
                    if is_dirty() {
                        span { class: "unsaved-indicator", title: "Unsaved changes" }
                    }
                }
                button {
                    class: "btn btn-success btn-sm",
                    disabled: !is_dirty() || is_saving(),
                    onclick: move |_| {
                        let notes_value = notes_text();
                        let notes_to_save = if notes_value.trim().is_empty() {
                            None
                        } else {
                            Some(notes_value.clone())
                        };
                        let boss_id = boss_id_btn.clone();
                        let file_path = file_path_btn.clone();

                        is_saving.set(true);
                        spawn(async move {
                            match api::update_boss_notes(&boss_id, &file_path, notes_to_save.clone()).await {
                                Ok(_) => {
                                    on_change.call(notes_to_save);
                                    is_dirty.set(false);
                                    on_status.call(("Notes saved".to_string(), false));
                                }
                                Err(e) => {
                                    on_status.call((format!("Failed to save: {}", e), true));
                                }
                            }
                            is_saving.set(false);
                        });
                    },
                    if is_saving() {
                        "Saving..."
                    } else {
                        "Save"
                    }
                }
            }

            // Help text
            div { class: "text-xs text-muted mb-sm",
                "Write notes in Markdown format. Supports: "
                code { "## headers" }
                ", "
                code { "**bold**" }
                ", "
                code { "*italic*" }
                ", "
                code { "- bullets" }
                ", "
                code { "1. numbered lists" }
                ", "
                code { "---" }
                " dividers"
            }

            // Textarea for notes
            textarea {
                class: "notes-textarea",
                placeholder: "## Tank Notes\n- Swap at 3 stacks\n\n## Healer Notes\n- Watch for raid damage\n\n## DPS Notes\n- Kill adds first",
                value: "{notes_text}",
                oninput: move |e| {
                    notes_text.set(e.value());
                    is_dirty.set(true);
                },
                // Save on Ctrl+S
                onkeydown: move |e: Event<KeyboardData>| {
                    if e.modifiers().ctrl() && e.key() == Key::Character("s".to_string()) {
                        e.prevent_default();
                        if is_dirty() && !is_saving() {
                            let notes_value = notes_text();
                            let notes_to_save = if notes_value.trim().is_empty() {
                                None
                            } else {
                                Some(notes_value.clone())
                            };
                            let boss_id = boss_id_kbd.clone();
                            let file_path = file_path_kbd.clone();

                            is_saving.set(true);
                            spawn(async move {
                                match api::update_boss_notes(&boss_id, &file_path, notes_to_save.clone()).await {
                                    Ok(_) => {
                                        on_change.call(notes_to_save);
                                        is_dirty.set(false);
                                        on_status.call(("Notes saved".to_string(), false));
                                    }
                                    Err(e) => {
                                        on_status.call((format!("Failed to save: {}", e), true));
                                    }
                                }
                                is_saving.set(false);
                            });
                        }
                    }
                },
            }

            // Character count
            div { class: "text-xs text-muted mt-xs text-right",
                "{notes_text().len()} characters"
            }
        }
    }
}
