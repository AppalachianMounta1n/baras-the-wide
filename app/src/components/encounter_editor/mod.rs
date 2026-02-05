//! Encounter Editor
//!
//! Full CRUD for the BossEncounter DSL: timers, phases, counters, challenges, entities.
//! Uses unified BossWithPath type and EncounterItem enum for streamlined data handling.

mod challenges;
mod conditions;
mod counters;
mod entities;
mod new_forms;
mod notes;
mod phases;
mod tabs;
mod timers;
pub mod triggers;

use dioxus::prelude::*;

// ─────────────────────────────────────────────────────────────────────────────
// Shared: Inline Name Creator
// ─────────────────────────────────────────────────────────────────────────────

/// Reusable inline name input component for creating new items.
/// Handles show/hide state internally. Calls `on_create` with the entered name.
#[component]
pub fn InlineNameCreator(
    button_label: &'static str,
    placeholder: &'static str,
    on_create: EventHandler<String>,
) -> Element {
    let mut show_input = use_signal(|| false);
    let mut name = use_signal(String::new);

    rsx! {
        if show_input() {
            div { class: "flex items-center gap-xs",
                input {
                    class: "input-inline",
                    r#type: "text",
                    placeholder: placeholder,
                    style: "width: 180px;",
                    value: "{name}",
                    autofocus: true,
                    oninput: move |e| name.set(e.value()),
                    onkeydown: move |e| {
                        if e.key() == Key::Enter && !name().is_empty() {
                            on_create.call(name());
                            show_input.set(false);
                            name.set(String::new());
                        } else if e.key() == Key::Escape {
                            show_input.set(false);
                            name.set(String::new());
                        }
                    }
                }
                button {
                    class: "btn btn-success btn-sm",
                    disabled: name().is_empty(),
                    onclick: move |_| {
                        if !name().is_empty() {
                            on_create.call(name());
                            show_input.set(false);
                            name.set(String::new());
                        }
                    },
                    "Create"
                }
                button {
                    class: "btn btn-ghost btn-sm",
                    onclick: move |_| {
                        show_input.set(false);
                        name.set(String::new());
                    },
                    "×"
                }
            }
        } else {
            button {
                class: "btn btn-success btn-sm",
                onclick: move |_| show_input.set(true),
                "{button_label}"
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared: NPC ID Chip Editor
// ─────────────────────────────────────────────────────────────────────────────

/// Chip editor for NPC IDs with +Add button
#[component]
pub fn NpcIdChipEditor(ids: Vec<i64>, on_change: EventHandler<Vec<i64>>) -> Element {
    let mut new_input = use_signal(String::new);
    let ids_for_keydown = ids.clone();
    let ids_for_click = ids.clone();

    rsx! {
        div { class: "flex-col gap-xs",
            // ID chips
            if !ids.is_empty() {
                div { class: "flex flex-wrap gap-xs mb-xs",
                    for (idx, id) in ids.iter().enumerate() {
                        {
                            let ids_clone = ids.clone();
                            rsx! {
                                span { class: "chip text-mono",
                                    "{id}"
                                    button {
                                        class: "chip-remove",
                                        onclick: move |_| {
                                            let mut new_ids = ids_clone.clone();
                                            new_ids.remove(idx);
                                            on_change.call(new_ids);
                                        },
                                        "×"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Add new ID
            div { class: "flex gap-xs",
                input {
                    r#type: "text",
                    class: "input-inline text-mono",
                    style: "width: 150px;",
                    placeholder: "NPC ID (Enter)",
                    value: "{new_input}",
                    oninput: move |e| new_input.set(e.value()),
                    onkeydown: move |e| {
                        if e.key() == Key::Enter && !new_input().trim().is_empty()
                            && let Ok(id) = new_input().trim().parse::<i64>() {
                                let mut new_ids = ids_for_keydown.clone();
                                if !new_ids.contains(&id) {
                                    new_ids.push(id);
                                    on_change.call(new_ids);
                                }
                                new_input.set(String::new());
                            }
                    }
                }
                button {
                    class: "btn btn-sm",
                    onclick: move |_| {
                        if let Ok(id) = new_input().trim().parse::<i64>() {
                            let mut new_ids = ids_for_click.clone();
                            if !new_ids.contains(&id) {
                                new_ids.push(id);
                                on_change.call(new_ids);
                            }
                            new_input.set(String::new());
                        }
                    },
                    "Add"
                }
            }
        }
    }
}

use crate::api;
use crate::types::{AreaListItem, BossWithPath, UiSessionState};

pub use tabs::BossTabs;

// ─────────────────────────────────────────────────────────────────────────────
// Main Panel
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
pub struct EncounterEditorProps {
    /// Unified UI session state (includes persisted state for this panel)
    pub state: Signal<UiSessionState>,
}

#[component]
pub fn EncounterEditorPanel(mut props: EncounterEditorProps) -> Element {
    // Area index state (not persisted - loaded fresh each time)
    let mut areas = use_signal(Vec::<AreaListItem>::new);
    let mut loading_areas = use_signal(|| true);

    // Boss state - unified: one signal holds all bosses with their items
    let mut bosses = use_signal(Vec::<BossWithPath>::new);
    let mut loading_bosses = use_signal(|| false);

    // Extract persisted state fields
    let mut selected_area_path = use_signal(|| props.state.read().encounter_builder.selected_area_path.clone());
    let mut selected_area_name = use_signal(|| props.state.read().encounter_builder.selected_area_name.clone());
    let mut expanded_boss = use_signal(|| props.state.read().encounter_builder.expanded_boss.clone());
    let mut area_filter = use_signal(|| props.state.read().encounter_builder.area_filter.clone());
    let active_boss_tab = use_signal(|| props.state.read().encounter_builder.active_boss_tab.clone());
    
    // Expanded items within each tab
    let expanded_timer = use_signal(|| props.state.read().encounter_builder.expanded_timer.clone());
    let expanded_phase = use_signal(|| props.state.read().encounter_builder.expanded_phase.clone());
    let expanded_counter = use_signal(|| props.state.read().encounter_builder.expanded_counter.clone());
    let expanded_challenge = use_signal(|| props.state.read().encounter_builder.expanded_challenge.clone());
    let expanded_entity = use_signal(|| props.state.read().encounter_builder.expanded_entity.clone());
    
    // Derived: selected_area AreaListItem (reconstructed from path/name when areas load)
    let mut selected_area = use_signal(|| None::<AreaListItem>);
    
    // Sync persisted state back to unified state
    use_effect(move || {
        let mut state = props.state.write();
        state.encounter_builder.selected_area_path = selected_area_path.read().clone();
        state.encounter_builder.selected_area_name = selected_area_name.read().clone();
        state.encounter_builder.expanded_boss = expanded_boss.read().clone();
        state.encounter_builder.area_filter = area_filter.read().clone();
        state.encounter_builder.active_boss_tab = active_boss_tab.read().clone();
        state.encounter_builder.expanded_timer = expanded_timer.read().clone();
        state.encounter_builder.expanded_phase = expanded_phase.read().clone();
        state.encounter_builder.expanded_counter = expanded_counter.read().clone();
        state.encounter_builder.expanded_challenge = expanded_challenge.read().clone();
        state.encounter_builder.expanded_entity = expanded_entity.read().clone();
    });
    
    // Non-persisted UI state
    let mut show_new_area = use_signal(|| false);
    let mut show_new_boss = use_signal(|| false);
    let mut status_message = use_signal(|| None::<(String, bool)>);

    // Auto-dismiss toast after 3 seconds
    use_effect(move || {
        if status_message().is_some() {
            spawn(async move {
                gloo_timers::future::TimeoutFuture::new(3000).await;
                status_message.set(None);
            });
        }
    });

    // Load area index on mount and restore selected area if persisted
    use_effect(move || {
        spawn(async move {
            if let Some(area_list) = api::get_area_index().await {
                areas.set(area_list.clone());
                
                // Restore selected area from persisted path
                if let Some(ref path) = *selected_area_path.read() {
                    if let Some(area) = area_list.iter().find(|a| &a.file_path == path) {
                        selected_area.set(Some(area.clone()));
                        // Load bosses for the restored area
                        if let Some(boss_list) = api::fetch_area_bosses(&area.file_path).await {
                            bosses.set(boss_list);
                        }
                    }
                }
            }
            loading_areas.set(false);
        });
    });

    // Load bosses when area is selected - single unified call
    let mut load_area_data = move |area: AreaListItem| {
        let file_path = area.file_path.clone();
        let area_name = area.name.clone();
        
        // Update persisted state
        selected_area_path.set(Some(file_path.clone()));
        selected_area_name.set(Some(area_name));
        selected_area.set(Some(area));
        
        loading_bosses.set(true);
        bosses.set(Vec::new());
        expanded_boss.set(None);

        spawn(async move {
            if let Some(b) = api::fetch_area_bosses(&file_path).await {
                bosses.set(b);
            }
            loading_bosses.set(false);
        });
    };

    // Group areas by category (with filtering)
    let grouped_areas = {
        let filter = area_filter().to_lowercase();
        let mut ops = Vec::new();
        let mut fps = Vec::new();
        let mut lairs = Vec::new();
        let mut other = Vec::new();

        for area in areas() {
            if !filter.is_empty() && !area.name.to_lowercase().contains(&filter) {
                continue;
            }
            match area.category.as_str() {
                "operations" => ops.push(area),
                "flashpoints" => fps.push(area),
                "lair_bosses" => lairs.push(area),
                _ => other.push(area),
            }
        }
        (ops, fps, lairs, other)
    };

    rsx! {
        div { class: "editor-layout",
            // ─── Sidebar: Area List ───────────────────────────────────────────
            div { class: "editor-sidebar",
                div { class: "editor-sidebar-header",
                    span { class: "text-sm text-muted", "Areas" }
                    button {
                        class: "btn btn-success btn-sm",
                        onclick: move |_| show_new_area.set(true),
                        "+ New"
                    }
                }

                div { class: "p-sm",
                    input {
                        class: "input input-sm w-full",
                        r#type: "text",
                        placeholder: "Filter...",
                        value: "{area_filter}",
                        oninput: move |e| area_filter.set(e.value())
                    }
                }

                div { class: "editor-sidebar-content",
                    if loading_areas() {
                        div { class: "empty-state text-sm", "Loading..." }
                    } else {
                        if !grouped_areas.0.is_empty() {
                            AreaCategory {
                                name: "Operations",
                                areas: grouped_areas.0.clone(),
                                selected: selected_area(),
                                on_select: move |a| load_area_data(a),
                            }
                        }
                        if !grouped_areas.1.is_empty() {
                            AreaCategory {
                                name: "Flashpoints",
                                areas: grouped_areas.1.clone(),
                                selected: selected_area(),
                                on_select: move |a| load_area_data(a),
                            }
                        }
                        if !grouped_areas.2.is_empty() {
                            AreaCategory {
                                name: "Lair Bosses",
                                areas: grouped_areas.2.clone(),
                                selected: selected_area(),
                                on_select: move |a| load_area_data(a),
                            }
                        }
                        if !grouped_areas.3.is_empty() {
                            AreaCategory {
                                name: "Other",
                                areas: grouped_areas.3.clone(),
                                selected: selected_area(),
                                on_select: move |a| load_area_data(a),
                            }
                        }
                    }
                }
            }

            // ─── Main Content ─────────────────────────────────────────────────
            div { class: "editor-main",
                if selected_area().is_none() {
                    div { class: "empty-state",
                        div { class: "empty-state-icon", "📂" }
                        "Select an area to edit encounters"
                    }
                } else if loading_bosses() {
                    div { class: "empty-state", "Loading..." }
                } else {
                    // Area header
                    div { class: "flex items-center justify-between mb-md",
                        h2 { class: "text-primary", "{selected_area().map(|a| a.name).unwrap_or_default()}" }
                        button {
                            class: "btn btn-success btn-sm",
                            onclick: move |_| show_new_boss.set(true),
                            "+ New Boss"
                        }
                    }

                    // New boss form
                    if show_new_boss() {
                        if let Some(area) = selected_area() {
                            {
                                let file_path = area.file_path.clone();
                                rsx! {
                                    new_forms::NewBossForm {
                                        area: area,
                                        on_create: move |new_boss| {
                                            let fp = file_path.clone();
                                            spawn(async move {
                                                match api::create_boss(&new_boss).await {
                                                    Ok(_) => {
                                                        // Reload area to get fresh BossWithPath
                                                        if let Some(b) = api::fetch_area_bosses(&fp).await {
                                                            bosses.set(b);
                                                        }
                                                        status_message.set(Some(("Boss created".to_string(), false)));
                                                    }
                                                    Err(e) => {
                                                        status_message.set(Some((e, true)));
                                                    }
                                                }
                                            });
                                            show_new_boss.set(false);
                                        },
                                        on_cancel: move |_| show_new_boss.set(false),
                                    }
                                }
                            }
                        }
                    }

                    // Boss list
                    if bosses().is_empty() {
                        div { class: "empty-state", "No bosses in this area" }
                    } else {
                        for bwp in bosses() {
                            {
                                let is_expanded = expanded_boss() == Some(bwp.boss.id.clone());
                                let boss_id = bwp.boss.id.clone();
                                // Extract counts directly from BossWithPath
                                let timer_count = bwp.boss.timers.len();
                                let phase_count = bwp.boss.phases.len();
                                let counter_count = bwp.boss.counters.len();
                                let challenge_count = bwp.boss.challenges.len();
                                let entity_count = bwp.boss.entities.len();

                                rsx! {
                                    div { class: "list-item",
                                        div {
                                            class: "list-item-header",
                                            onclick: move |_| {
                                                expanded_boss.set(if is_expanded { None } else { Some(boss_id.clone()) });
                                            },
                                            span { class: "list-item-expand", if is_expanded { "▼" } else { "▶" } }
                                            span { class: "font-medium text-primary", "{bwp.boss.name}" }
                                            span { class: "text-xs text-mono text-muted", "{bwp.boss.id}" }
                                            if timer_count > 0 {
                                                span { class: "tag", "{timer_count} timers" }
                                            }
                                            if phase_count > 0 {
                                                span { class: "tag", "{phase_count} phases" }
                                            }
                                            if counter_count > 0 {
                                                span { class: "tag", "{counter_count} counters" }
                                            }
                                            if challenge_count > 0 {
                                                span { class: "tag", "{challenge_count} challenges" }
                                            }
                                            if entity_count > 0 {
                                                span { class: "tag", "{entity_count} entities" }
                                            }
                                        }

                                        if is_expanded {
                                            div { class: "list-item-body",
                                                BossTabs {
                                                    boss_with_path: bwp.clone(),
                                                    active_tab: active_boss_tab,
                                                    expanded_timer: expanded_timer,
                                                    expanded_phase: expanded_phase,
                                                    expanded_counter: expanded_counter,
                                                    expanded_challenge: expanded_challenge,
                                                    expanded_entity: expanded_entity,
                                                    on_boss_change: move |updated: BossWithPath| {
                                                        let mut all = bosses();
                                                        if let Some(idx) = all.iter().position(|b| b.boss.id == updated.boss.id) {
                                                            all[idx] = updated;
                                                            bosses.set(all);
                                                        }
                                                    },
                                                    on_status: move |msg| status_message.set(Some(msg)),
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // New Area modal
        if show_new_area() {
            new_forms::NewAreaForm {
                on_create: move |new_area| {
                    spawn(async move {
                        match api::create_area(&new_area).await {
                            Ok(_) => {
                                if let Some(a) = api::get_area_index().await {
                                    areas.set(a);
                                }
                                status_message.set(Some(("Area created".to_string(), false)));
                            }
                            Err(e) => {
                                status_message.set(Some((e, true)));
                            }
                        }
                    });
                    show_new_area.set(false);
                },
                on_cancel: move |_| show_new_area.set(false),
            }
        }

        // Toast notification (fixed bottom-right)
        if let Some((msg, is_error)) = status_message() {
            div {
                class: "toast",
                style: "position: fixed; bottom: 20px; right: 20px; z-index: 1000; \
                        padding: 12px 16px; border-radius: 6px; \
                        background: #2a2a2e; border: 1px solid #3a3a3e; \
                        box-shadow: 0 4px 12px rgba(0,0,0,0.5); \
                        display: flex; align-items: center; gap: 12px;",
                span {
                    style: if is_error { "color: var(--color-error);" } else { "color: var(--color-success);" },
                    if is_error { "✗" } else { "✓" }
                }
                span { "{msg}" }
                button {
                    class: "btn btn-ghost btn-sm",
                    style: "padding: 2px 6px; min-width: auto;",
                    onclick: move |_| status_message.set(None),
                    "×"
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Area Category (collapsible)
// ─────────────────────────────────────────────────────────────────────────────

#[component]
fn AreaCategory(
    name: &'static str,
    areas: Vec<AreaListItem>,
    selected: Option<AreaListItem>,
    on_select: EventHandler<AreaListItem>,
) -> Element {
    let mut collapsed = use_signal(|| false);

    rsx! {
        div { class: "category-group",
            div {
                class: "category-header",
                onclick: move |_| collapsed.set(!collapsed()),
                span { if collapsed() { "▶" } else { "▼" } }
                span { "{name}" }
                span { class: "sidebar-item-count", "{areas.len()}" }
            }

            if !collapsed() {
                div { class: "category-items",
                    for area in areas {
                        {
                            let is_active = selected.as_ref().map(|s| s.file_path == area.file_path).unwrap_or(false);
                            let area_clone = area.clone();

                            rsx! {
                                div {
                                    class: if is_active { "sidebar-item active" } else { "sidebar-item" },
                                    onclick: move |_| on_select.call(area_clone.clone()),
                                    "{area.name}"
                                    span { class: "sidebar-item-count", "{area.boss_count}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
