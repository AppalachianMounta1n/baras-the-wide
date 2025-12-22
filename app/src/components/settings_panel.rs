//! Settings panel component for overlay configuration
//!
//! This is a floating, draggable panel that allows users to customize
//! overlay appearances, personal stats, and (soon) raid frame settings.

use dioxus::prelude::*;
use crate::app::{
    MetricType, OverlayAppearanceConfig, OverlaySettings, OverlayStatus,
    PersonalOverlayConfig, PersonalStat, RaidOverlaySettings, parse_hex_color,
    MAX_PROFILES,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

use crate::app::AppConfig;

#[component]
pub fn SettingsPanel(
    settings: Signal<OverlaySettings>,
    selected_tab: Signal<String>,
    profile_names: Signal<Vec<String>>,
    active_profile: Signal<Option<String>>,
    // Overlay enabled signals for UI state updates
    metric_overlays_enabled: Signal<std::collections::HashMap<MetricType, bool>>,
    personal_enabled: Signal<bool>,
    raid_enabled: Signal<bool>,
    overlays_visible: Signal<bool>,
    on_close: EventHandler<()>,
    on_header_mousedown: EventHandler<MouseEvent>,
) -> Element {
    // Local draft of settings being edited
    let mut draft_settings = use_signal(|| settings());
    let mut has_changes = use_signal(|| false);
    let mut save_status = use_signal(String::new);

    // Profile UI state (local to this panel)
    let mut new_profile_name = use_signal(String::new);
    let mut profile_status = use_signal(String::new);

    let current_settings = draft_settings();
    let tab = selected_tab();

    // Get appearance for current tab (uses backend-provided defaults if no saved appearance)
    let get_appearance = |key: &str| -> OverlayAppearanceConfig {
        current_settings.appearances.get(key).cloned()
            .or_else(|| current_settings.default_appearances.get(key).cloned())
            .unwrap_or_default()
    };

    let current_appearance = get_appearance(&tab);

    // Pre-compute hex color strings for color pickers
    let bar_color_hex = format!(
        "#{:02x}{:02x}{:02x}",
        current_appearance.bar_color[0],
        current_appearance.bar_color[1],
        current_appearance.bar_color[2]
    );
    let font_color_hex = format!(
        "#{:02x}{:02x}{:02x}",
        current_appearance.font_color[0],
        current_appearance.font_color[1],
        current_appearance.font_color[2]
    );
    let personal_font_color_hex = format!(
        "#{:02x}{:02x}{:02x}",
        current_settings.personal_overlay.font_color[0],
        current_settings.personal_overlay.font_color[1],
        current_settings.personal_overlay.font_color[2]
    );

    let personal_label_font_color_hex = format!(
        "#{:02x}{:02x}{:02x}",
        current_settings.personal_overlay.label_color[0],
        current_settings.personal_overlay.label_color[1],
        current_settings.personal_overlay.label_color[2]
    );


    // Save settings to backend (preserves positions)
    let save_to_backend = move |_| {
        let new_settings = draft_settings();
        async move {
            // Get current full config first to preserve positions
            let result = invoke("get_config", JsValue::NULL).await;
            if let Ok(mut config) = serde_wasm_bindgen::from_value::<AppConfig>(result) {
                // Preserve existing positions - only update appearances and other settings
                let existing_positions = config.overlay_settings.positions.clone();
                let existing_enabled = config.overlay_settings.enabled.clone();

                config.overlay_settings.appearances = new_settings.appearances.clone();
                config.overlay_settings.personal_overlay = new_settings.personal_overlay.clone();
                config.overlay_settings.metric_opacity = new_settings.metric_opacity;
                config.overlay_settings.personal_opacity = new_settings.personal_opacity;
                config.overlay_settings.raid_overlay = new_settings.raid_overlay.clone();
                config.overlay_settings.raid_opacity = new_settings.raid_opacity;
                // Keep positions and enabled state untouched
                config.overlay_settings.positions = existing_positions;
                config.overlay_settings.enabled = existing_enabled;

                let args = serde_wasm_bindgen::to_value(&config).unwrap_or(JsValue::NULL);
                let obj = js_sys::Object::new();
                js_sys::Reflect::set(&obj, &JsValue::from_str("config"), &args).unwrap();

                let result = invoke("update_config", obj.into()).await;
                if result.is_undefined() || result.is_null() {
                    // Refresh running overlays with new settings
                    let _ = invoke("refresh_overlay_settings", JsValue::NULL).await;

                    settings.set(new_settings);
                    has_changes.set(false);
                    save_status.set("Settings saved!".to_string());
                } else {
                    save_status.set("Failed to save".to_string());
                }
            }
        }
    };

    // Update draft settings helper
    let mut update_draft = move |new_settings: OverlaySettings| {
        draft_settings.set(new_settings);
        has_changes.set(true);
        save_status.set(String::new());
    };

    rsx! {
        section { class: "settings-panel",
            div {
                class: "settings-header draggable",
                onmousedown: move |e| on_header_mousedown.call(e),
                h3 { "Overlay Settings" }
                button {
                    class: "btn btn-close",
                    onclick: move |_| on_close.call(()),
                    onmousedown: move |e| e.stop_propagation(), // Don't start drag when clicking close
                    "X"
                }
            }

            // Profiles section (collapsible accordion)
            details { class: "settings-section collapsible",
                summary { class: "collapsible-summary",
                    i { class: "fa-solid fa-user-gear summary-icon" }
                    "Profiles"
                    if let Some(ref name) = active_profile() {
                        span { class: "profile-active-badge", "{name}" }
                    }
                }
                div { class: "collapsible-content",
                    // Profile list
                    if !profile_names().is_empty() {
                        div { class: "profile-list compact",
                            for name in profile_names().iter() {
                                {
                                    let profile_name = name.clone();
                                    let is_active = active_profile().as_ref() == Some(&profile_name);
                                    rsx! {
                                        div {
                                            class: if is_active { "profile-item active" } else { "profile-item" },
                                            span { class: "profile-name", "{profile_name}" }
                                            div { class: "profile-actions",
                                                // Load button
                                                button {
                                                    class: "btn btn-small btn-load",
                                                    disabled: is_active,
                                                    onclick: {
                                                        let pname = profile_name.clone();
                                                        move |_| {
                                                            let pname = pname.clone();
                                                            spawn(async move {
                                                                let obj = js_sys::Object::new();
                                                                js_sys::Reflect::set(&obj, &JsValue::from_str("name"), &JsValue::from_str(&pname)).unwrap();
                                                                let result = invoke("load_profile", obj.into()).await;
                                                                if result.is_undefined() || result.is_null() {
                                                                    active_profile.set(Some(pname.clone()));
                                                                    profile_status.set(format!("Loaded '{}'", pname));
                                                                    // Refresh settings in this panel
                                                                    let config_result = invoke("get_config", JsValue::NULL).await;
                                                                    if let Ok(config) = serde_wasm_bindgen::from_value::<AppConfig>(config_result) {
                                                                        draft_settings.set(config.overlay_settings.clone());
                                                                        settings.set(config.overlay_settings);
                                                                    }
                                                                    // Refresh ALL running overlays
                                                                    let _ = invoke("refresh_overlay_settings", JsValue::NULL).await;
                                                                    // Update UI button states from actual overlay status
                                                                    let status_result = invoke("get_overlay_status", JsValue::NULL).await;
                                                                    if let Ok(status) = serde_wasm_bindgen::from_value::<OverlayStatus>(status_result) {
                                                                        let mut new_map = std::collections::HashMap::new();
                                                                        for ot in MetricType::all_metrics() {
                                                                            let key = ot.config_key().to_string();
                                                                            new_map.insert(*ot, status.enabled.contains(&key));
                                                                        }
                                                                        metric_overlays_enabled.set(new_map);
                                                                        personal_enabled.set(status.personal_enabled);
                                                                        raid_enabled.set(status.raid_enabled);
                                                                        overlays_visible.set(status.overlays_visible);
                                                                    }
                                                                } else if let Some(err) = result.as_string() {
                                                                    profile_status.set(format!("Error: {}", err));
                                                                }
                                                            });
                                                        }
                                                    },
                                                    "Load"
                                                }
                                                // Save/Update button
                                                button {
                                                    class: "btn btn-small btn-update",
                                                    title: "Overwrite profile with current settings",
                                                    onclick: {
                                                        let pname = profile_name.clone();
                                                        move |_| {
                                                            let pname = pname.clone();
                                                            spawn(async move {
                                                                let obj = js_sys::Object::new();
                                                                js_sys::Reflect::set(&obj, &JsValue::from_str("name"), &JsValue::from_str(&pname)).unwrap();
                                                                let result = invoke("save_profile", obj.into()).await;
                                                                if result.is_undefined() || result.is_null() {
                                                                    active_profile.set(Some(pname.clone()));
                                                                    profile_status.set(format!("Saved '{}'", pname));
                                                                } else if let Some(err) = result.as_string() {
                                                                    profile_status.set(format!("Error: {}", err));
                                                                }
                                                            });
                                                        }
                                                    },
                                                    "Save"
                                                }
                                                // Delete button
                                                button {
                                                    class: "btn btn-small btn-delete",
                                                    onclick: {
                                                        let pname = profile_name.clone();
                                                        move |_| {
                                                            let pname = pname.clone();
                                                            spawn(async move {
                                                                let obj = js_sys::Object::new();
                                                                js_sys::Reflect::set(&obj, &JsValue::from_str("name"), &JsValue::from_str(&pname)).unwrap();
                                                                let result = invoke("delete_profile", obj.into()).await;
                                                                if result.is_undefined() || result.is_null() {
                                                                    // Refresh profile list in shared state
                                                                    let names_result = invoke("get_profile_names", JsValue::NULL).await;
                                                                    if let Ok(names) = serde_wasm_bindgen::from_value::<Vec<String>>(names_result) {
                                                                        profile_names.set(names);
                                                                    }
                                                                    let active_result = invoke("get_active_profile", JsValue::NULL).await;
                                                                    if let Ok(name) = serde_wasm_bindgen::from_value::<Option<String>>(active_result) {
                                                                        active_profile.set(name);
                                                                    }
                                                                    profile_status.set(format!("Deleted '{}'", pname));
                                                                } else if let Some(err) = result.as_string() {
                                                                    profile_status.set(format!("Error: {}", err));
                                                                }
                                                            });
                                                        }
                                                    },
                                                    "×"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Create new profile
                    div { class: "profile-create",
                        input {
                            r#type: "text",
                            class: "profile-name-input",
                            placeholder: "New profile name...",
                            maxlength: "32",
                            value: new_profile_name,
                            oninput: move |e| new_profile_name.set(e.value())
                        }
                        button {
                            class: "btn btn-small btn-save",
                            disabled: new_profile_name().trim().is_empty() || profile_names().len() >= MAX_PROFILES,
                            onclick: move |_| {
                                let name = new_profile_name().trim().to_string();
                                if name.is_empty() { return; }

                                spawn(async move {
                                    let obj = js_sys::Object::new();
                                    js_sys::Reflect::set(&obj, &JsValue::from_str("name"), &JsValue::from_str(&name)).unwrap();
                                    let result = invoke("save_profile", obj.into()).await;
                                    if result.is_undefined() || result.is_null() {
                                        // Refresh profile list in shared state
                                        let names_result = invoke("get_profile_names", JsValue::NULL).await;
                                        if let Ok(names) = serde_wasm_bindgen::from_value::<Vec<String>>(names_result) {
                                            profile_names.set(names);
                                        }
                                        active_profile.set(Some(name.clone()));
                                        new_profile_name.set(String::new());
                                        profile_status.set(format!("Created '{}'", name));
                                    } else if let Some(err) = result.as_string() {
                                        profile_status.set(format!("Error: {}", err));
                                    }
                                });
                            },
                            "+ New"
                        }
                    }

                    if profile_names().len() >= MAX_PROFILES {
                        p { class: "hint hint-warning compact",
                            "Maximum {MAX_PROFILES} profiles"
                        }
                    }

                    if !profile_status().is_empty() {
                        p { class: "profile-status compact", "{profile_status}" }
                    }
                }
            }

            // Category opacity settings (collapsible)
            details { class: "settings-section collapsible",
                summary { class: "collapsible-summary", "Background Opacity" }
                div { class: "collapsible-content",
                    div { class: "setting-row",
                        label { "Metrics Opacity" }
                        input {
                            r#type: "range",
                            min: "0",
                            max: "255",
                            value: "{current_settings.metric_opacity}",
                            oninput: move |e| {
                                if let Ok(val) = e.value().parse::<u8>() {
                                    let mut new_settings = draft_settings();
                                    new_settings.metric_opacity = val;
                                    update_draft(new_settings);
                                }
                            }
                        }
                        span { class: "value", "{current_settings.metric_opacity}" }
                    }
                    div { class: "setting-row",
                        label { "Personal Opacity" }
                        input {
                            r#type: "range",
                            min: "0",
                            max: "255",
                            value: "{current_settings.personal_opacity}",
                            oninput: move |e| {
                                if let Ok(val) = e.value().parse::<u8>() {
                                    let mut new_settings = draft_settings();
                                    new_settings.personal_opacity = val;
                                    update_draft(new_settings);
                                }
                            }
                        }
                        span { class: "value", "{current_settings.personal_opacity}" }
                    }
                }
            }

            // Tabs for overlay types - grouped by category
            div { class: "settings-tabs",
                // General group
                div { class: "tab-group",
                    span { class: "tab-group-label", "General" }
                    div { class: "tab-group-buttons",
                        button {
                            class: if tab == "personal" { "tab-btn active" } else { "tab-btn" },
                            onclick: move |_| selected_tab.set("personal".to_string()),
                            "Personal Stats"
                        }
                        button {
                            class: if tab == "raid" { "tab-btn active" } else { "tab-btn" },
                            onclick: move |_| selected_tab.set("raid".to_string()),
                            "Raid Frames"
                        }
                    }
                }
                // Metrics group
                div { class: "tab-group",
                    span { class: "tab-group-label", "Metrics" }
                    div { class: "tab-group-buttons",
                        for overlay_type in MetricType::all_metrics() {
                            {
                                let ot = *overlay_type;
                                let key = ot.config_key().to_string();
                                let label = ot.label();
                                rsx! {
                                    button {
                                        class: if tab == key { "tab-btn active" } else { "tab-btn" },
                                        onclick: move |_| selected_tab.set(key.clone()),
                                        "{label}"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Per-overlay settings
            if tab == "raid" {
                // Raid frame overlay settings
                div { class: "settings-section",
                    h4 { "Grid Layout" }

                    // Grid validation helper
                    {
                        let cols = current_settings.raid_overlay.grid_columns;
                        let rows = current_settings.raid_overlay.grid_rows;
                        let is_valid = current_settings.raid_overlay.is_valid_grid();

                        rsx! {
                            div { class: "setting-row",
                                label { "Columns" }
                                select {
                                    value: "{cols}",
                                    onchange: move |e: Event<FormData>| {
                                        if let Ok(val) = e.value().parse::<u8>() {
                                            let mut new_settings = draft_settings();
                                            new_settings.raid_overlay.grid_columns = val.clamp(1, 4);
                                            update_draft(new_settings);
                                        }
                                    },
                                    option { value: "1", "1" }
                                    option { value: "2", "2" }
                                    option { value: "4", "4" }
                                }
                            }

                            div { class: "setting-row",
                                label { "Rows" }
                                select {
                                    value: "{rows}",
                                    onchange: move |e: Event<FormData>| {
                                        if let Ok(val) = e.value().parse::<u8>() {
                                            let mut new_settings = draft_settings();
                                            new_settings.raid_overlay.grid_rows = val.clamp(1, 8);
                                            update_draft(new_settings);
                                        }
                                    },
                                    option { value: "1", "1" }
                                    option { value: "2", "2" }
                                    option { value: "4", "4" }
                                    option { value: "8", "8" }
                                }
                            }

                            // Grid validation message
                            div { class: "setting-row",
                                span { class: "hint",
                                    "Total slots: {cols * rows}"
                                }
                            }
                            if !is_valid {
                                div { class: "setting-row validation-error",
                                    "⚠ Grid must have 4, 8, or 16 total slots"
                                }
                            }

                            // Hint about requiring toggle
                            div { class: "setting-row",
                                span { class: "hint hint-subtle",
                                    "Grid changes require toggling overlay off/on"
                                }
                            }
                        }
                    }

                    h4 { "Appearance" }

                    // Background Opacity
                    div { class: "setting-row",
                        label { "Background Opacity" }
                        input {
                            r#type: "range",
                            min: "0",
                            max: "255",
                            value: "{current_settings.raid_opacity}",
                            oninput: move |e| {
                                if let Ok(val) = e.value().parse::<u8>() {
                                    let mut new_settings = draft_settings();
                                    new_settings.raid_opacity = val;
                                    update_draft(new_settings);
                                }
                            }
                        }
                        span { class: "value", "{current_settings.raid_opacity}" }
                    }

                    // Max Effects per Frame
                    div { class: "setting-row",
                        label { "Max Effects per Frame" }
                        input {
                            r#type: "number",
                            min: "1",
                            max: "8",
                            value: "{current_settings.raid_overlay.max_effects_per_frame}",
                            onchange: move |e: Event<FormData>| {
                                if let Ok(val) = e.value().parse::<u8>() {
                                    let mut new_settings = draft_settings();
                                    new_settings.raid_overlay.max_effects_per_frame = val.clamp(1, 8);
                                    update_draft(new_settings);
                                }
                            }
                        }
                    }

                    // Effect Size slider
                    div { class: "setting-row",
                        label { "Effect Size" }
                        input {
                            r#type: "range",
                            min: "8",
                            max: "24",
                            value: "{current_settings.raid_overlay.effect_size as i32}",
                            oninput: move |e| {
                                if let Ok(val) = e.value().parse::<f32>() {
                                    let mut new_settings = draft_settings();
                                    new_settings.raid_overlay.effect_size = val.clamp(8.0, 24.0);
                                    update_draft(new_settings);
                                }
                            }
                        }
                        span { class: "value", "{current_settings.raid_overlay.effect_size:.0}px" }
                    }

                    // Effect Vertical Position
                    div { class: "setting-row",
                        label { "Effect Vertical Offset" }
                        input {
                            r#type: "range",
                            min: "-10",
                            max: "30",
                            value: "{current_settings.raid_overlay.effect_vertical_offset as i32}",
                            oninput: move |e| {
                                if let Ok(val) = e.value().parse::<f32>() {
                                    let mut new_settings = draft_settings();
                                    new_settings.raid_overlay.effect_vertical_offset = val.clamp(-10.0, 30.0);
                                    update_draft(new_settings);
                                }
                            }
                        }
                        span { class: "value", "{current_settings.raid_overlay.effect_vertical_offset:.0}px" }
                    }

                    // Show Role Icons
                    div { class: "setting-row",
                        label { "Show Role Icons" }
                        input {
                            r#type: "checkbox",
                            checked: current_settings.raid_overlay.show_role_icons,
                            onchange: move |e: Event<FormData>| {
                                let mut new_settings = draft_settings();
                                new_settings.raid_overlay.show_role_icons = e.checked();
                                update_draft(new_settings);
                            }
                        }
                    }

                    // Reset to default button
                    div { class: "setting-row reset-row",
                        button {
                            class: "btn btn-reset",
                            onclick: move |_| {
                                let mut new_settings = draft_settings();
                                new_settings.raid_overlay = RaidOverlaySettings::default();
                                new_settings.raid_opacity = 180; // default opacity
                                update_draft(new_settings);
                            },
                            i { class: "fa-solid fa-rotate-left" }
                            span { " Reset Style" }
                        }
                    }
                }
            } else if tab != "personal" {
                div { class: "settings-section",
                    // Display options
                    div { class: "setting-row",
                        label { "Show Per-Second" }
                        input {
                            r#type: "checkbox",
                            checked: current_appearance.show_per_second,
                            onchange: {
                                let tab = tab.clone();
                                move |e: Event<FormData>| {
                                    let mut new_settings = draft_settings();
                                    let default = new_settings.default_appearances.get(&tab).cloned().unwrap_or_default();
                                    let mut appearance = new_settings.appearances
                                        .entry(tab.clone())
                                        .or_insert(default)
                                        .clone();
                                    appearance.show_per_second = e.checked();
                                    new_settings.appearances.insert(tab.clone(), appearance);
                                    update_draft(new_settings);
                                }
                            }
                        }
                    }

                    div { class: "setting-row",
                        label { "Show Total" }
                        input {
                            r#type: "checkbox",
                            checked: current_appearance.show_total,
                            onchange: {
                                let tab = tab.clone();
                                move |e: Event<FormData>| {
                                    let mut new_settings = draft_settings();
                                    let default = new_settings.default_appearances.get(&tab).cloned().unwrap_or_default();
                                    let mut appearance = new_settings.appearances
                                        .entry(tab.clone())
                                        .or_insert(default)
                                        .clone();
                                    appearance.show_total = e.checked();
                                    new_settings.appearances.insert(tab.clone(), appearance);
                                    update_draft(new_settings);
                                }
                            }
                        }
                    }

                    div { class: "setting-row",
                        label { "Show Header" }
                        input {
                            r#type: "checkbox",
                            checked: current_appearance.show_header,
                            onchange: {
                                let tab = tab.clone();
                                move |e: Event<FormData>| {
                                    let mut new_settings = draft_settings();
                                    let default = new_settings.default_appearances.get(&tab).cloned().unwrap_or_default();
                                    let mut appearance = new_settings.appearances
                                        .entry(tab.clone())
                                        .or_insert(default)
                                        .clone();
                                    appearance.show_header = e.checked();
                                    new_settings.appearances.insert(tab.clone(), appearance);
                                    update_draft(new_settings);
                                }
                            }
                        }
                    }

                    div { class: "setting-row",
                        label { "Show Footer" }
                        input {
                            r#type: "checkbox",
                            checked: current_appearance.show_footer,
                            onchange: {
                                let tab = tab.clone();
                                move |e: Event<FormData>| {
                                    let mut new_settings = draft_settings();
                                    let default = new_settings.default_appearances.get(&tab).cloned().unwrap_or_default();
                                    let mut appearance = new_settings.appearances
                                        .entry(tab.clone())
                                        .or_insert(default)
                                        .clone();
                                    appearance.show_footer = e.checked();
                                    new_settings.appearances.insert(tab.clone(), appearance);
                                    update_draft(new_settings);
                                }
                            }
                        }
                    }

                    div { class: "setting-row",
                        label { "Max Entries" }
                        input {
                            r#type: "number",
                            min: "1",
                            max: "16",
                            value: "{current_appearance.max_entries}",
                            onchange: {
                                let tab = tab.clone();
                                move |e: Event<FormData>| {
                                    if let Ok(val) = e.value().parse::<u8>() {
                                        let mut new_settings = draft_settings();
                                        let default = new_settings.default_appearances.get(&tab).cloned().unwrap_or_default();
                                        let mut appearance = new_settings.appearances
                                            .entry(tab.clone())
                                            .or_insert(default)
                                            .clone();
                                        appearance.max_entries = val.clamp(1, 16);
                                        new_settings.appearances.insert(tab.clone(), appearance);
                                        update_draft(new_settings);
                                    }
                                }
                            }
                        }
                    }

                    // Color settings
                    div { class: "setting-row",
                        label { "Bar Color" }
                        input {
                            r#type: "color",
                            key: "{tab}-bar",
                            value: "{bar_color_hex}",
                            class: "color-picker",
                            oninput: {
                                let tab = tab.clone();
                                move |e: Event<FormData>| {
                                    if let Some(color) = parse_hex_color(&e.value()) {
                                        let mut new_settings = draft_settings();
                                        let default = new_settings.default_appearances.get(&tab).cloned().unwrap_or_default();
                                        let mut appearance = new_settings.appearances
                                            .entry(tab.clone())
                                            .or_insert(default)
                                            .clone();
                                        appearance.bar_color = color;
                                        new_settings.appearances.insert(tab.clone(), appearance);
                                        update_draft(new_settings);
                                    }
                                }
                            }
                        }
                    }

                    div { class: "setting-row",
                        label { "Font Color" }
                        input {
                            r#type: "color",
                            key: "{tab}-font",
                            value: "{font_color_hex}",
                            class: "color-picker",
                            oninput: {
                                let tab = tab.clone();
                                move |e: Event<FormData>| {
                                    if let Some(color) = parse_hex_color(&e.value()) {
                                        let mut new_settings = draft_settings();
                                        let default = new_settings.default_appearances.get(&tab).cloned().unwrap_or_default();
                                        let mut appearance = new_settings.appearances
                                            .entry(tab.clone())
                                            .or_insert(default)
                                            .clone();
                                        appearance.font_color = color;
                                        new_settings.appearances.insert(tab.clone(), appearance);
                                        update_draft(new_settings);
                                    }
                                }
                            }
                        }
                    }

                    // Reset to default button
                    div { class: "setting-row reset-row",
                        button {
                            class: "btn btn-reset",
                            onclick: {
                                let tab = tab.clone();
                                move |_| {
                                    let mut new_settings = draft_settings();
                                    // Use overlay-specific default from backend
                                    let default_appearance = new_settings.default_appearances
                                        .get(&tab)
                                        .cloned()
                                        .unwrap_or_default();
                                    new_settings.appearances.insert(tab.clone(), default_appearance);
                                    update_draft(new_settings);
                                }
                            },
                            i { class: "fa-solid fa-rotate-left" }
                            span { " Reset Style" }
                        }
                    }
                }
            } else {
                // Personal overlay settings
                div { class: "settings-section",
                    p { class: "hint", "Displayed stats:" }

                    // Ordered list of selected stats
                    {
                        let visible_stats = current_settings.personal_overlay.visible_stats.clone();
                        let stat_count = visible_stats.len();
                        rsx! {
                            div { class: "stat-order-list",
                                for (idx, stat) in visible_stats.into_iter().enumerate() {
                                    div { class: "stat-order-item", key: "{stat:?}",
                                        span { class: "stat-name", "{stat.label()}" }
                                        div { class: "stat-controls",
                                            button {
                                                class: "btn-order",
                                                disabled: idx == 0,
                                                onclick: move |_| {
                                                    let mut new_settings = draft_settings();
                                                    let stats = &mut new_settings.personal_overlay.visible_stats;
                                                    if idx > 0 {
                                                        stats.swap(idx, idx - 1);
                                                    }
                                                    update_draft(new_settings);
                                                },
                                                "▲"
                                            }
                                            button {
                                                class: "btn-order",
                                                disabled: idx >= stat_count - 1,
                                                onclick: move |_| {
                                                    let mut new_settings = draft_settings();
                                                    let stats = &mut new_settings.personal_overlay.visible_stats;
                                                    if idx < stats.len() - 1 {
                                                        stats.swap(idx, idx + 1);
                                                    }
                                                    update_draft(new_settings);
                                                },
                                                "▼"
                                            }
                                            button {
                                                class: "btn-remove",
                                                onclick: move |_| {
                                                    let mut new_settings = draft_settings();
                                                    new_settings.personal_overlay.visible_stats.retain(|s| *s != stat);
                                                    update_draft(new_settings);
                                                },
                                                "✕"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Available stats to add
                    div { class: "stat-add-section",
                        p { class: "hint", "Add stats:" }
                        div { class: "stat-add-grid",
                            for stat in PersonalStat::all() {
                                {
                                    let is_visible = current_settings.personal_overlay.visible_stats.contains(stat);
                                    if !is_visible {
                                        let stat = *stat;
                                        rsx! {
                                            button {
                                                class: "btn-add-stat",
                                                onclick: move |_| {
                                                    let mut new_settings = draft_settings();
                                                    if !new_settings.personal_overlay.visible_stats.contains(&stat) {
                                                        new_settings.personal_overlay.visible_stats.push(stat);
                                                    }
                                                    update_draft(new_settings);
                                                },
                                                "+ {stat.label()}"
                                            }
                                        }
                                    } else {
                                        rsx! {}
                                    }
                                }
                            }
                        }
                    }

                    // Personal overlay font color
                    div { class: "setting-row",
                        label { "Value Font Color" }
                        input {
                            r#type: "color",
                            key: "personal-font",
                            value: "{personal_font_color_hex}",
                            class: "color-picker",
                            oninput: move |e: Event<FormData>| {
                                if let Some(color) = parse_hex_color(&e.value()) {
                                    let mut new_settings = draft_settings();
                                    new_settings.personal_overlay.font_color = color;
                                    update_draft(new_settings);
                                }
                            }
                        }
                    }

                    div { class: "setting-row",
                        label { "Label Font Color" }
                        input {
                            r#type: "color",
                            key: "personal-label-font",
                            value: "{personal_label_font_color_hex}",
                            class: "color-picker",
                            oninput: move |e: Event<FormData>| {
                                if let Some(color) = parse_hex_color(&e.value()) {
                                    let mut new_settings = draft_settings();
                                    new_settings.personal_overlay.label_color = color;
                                    update_draft(new_settings);
                                }
                            }
                        }
                    }
                    // Reset to default button
                    div { class: "setting-row reset-row",
                        button {
                            class: "btn btn-reset",
                            onclick: move |_| {
                                let mut new_settings = draft_settings();
                                new_settings.personal_overlay = PersonalOverlayConfig::default();
                                update_draft(new_settings);
                            },
                            i { class: "fa-solid fa-rotate-left" }
                            span { " Reset Style" }
                        }
                    }
                }
            }

            // Save button and status
            div { class: "settings-footer",
                button {
                    class: if has_changes() { "btn btn-save" } else { "btn btn-save btn-disabled" },
                    disabled: !has_changes(),
                    onclick: save_to_backend,
                    "Save Settings"
                }
                if !save_status().is_empty() {
                    span { class: "save-status", "{save_status()}" }
                }
            }
        }
    }
}
