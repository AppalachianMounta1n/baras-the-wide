//! Rotation visualization component.
//!
//! Displays ability rotation cycles split by an anchor ability,
//! with GCD abilities in a horizontal row and off-GCD weaves stacked above.

use dioxus::prelude::*;

use crate::api;
use crate::api::{RotationAnalysis, TimeRange};
use crate::components::ability_icon::AbilityIcon;

#[derive(Props, Clone, PartialEq)]
pub struct RotationViewProps {
    pub encounter_idx: Option<u32>,
    pub time_range: TimeRange,
    pub selected_source: Option<String>,
    /// Optional callback to update the parent's time range (e.g. from context menu)
    #[props(default)]
    pub on_range_change: Option<EventHandler<TimeRange>>,
}

#[component]
pub fn RotationView(props: RotationViewProps) -> Element {
    let mut available_abilities = use_signal(|| Vec::<(i64, String)>::new());
    let mut selected_anchor = use_signal(|| None::<i64>);
    let mut rotation = use_signal(|| None::<RotationAnalysis>);
    let mut loading = use_signal(|| false);

    // Context menu state for right-click "Set as range start/end"
    let mut context_menu_pos = use_signal(|| None::<(f64, f64)>);
    let mut context_menu_time = use_signal(|| 0.0f32);

    // Track source in a signal so effects can react to changes
    let mut tracked_source = use_signal(|| props.selected_source.clone());
    if *tracked_source.read() != props.selected_source {
        tracked_source.set(props.selected_source.clone());
    }

    // Track time_range so effects react to phase/time filter changes
    let mut tracked_time_range = use_signal(|| props.time_range.clone());
    if *tracked_time_range.read() != props.time_range {
        tracked_time_range.set(props.time_range.clone());
    }

    // Flag: user has clicked Create (reset on source/anchor change)
    let mut rotation_active = use_signal(|| false);

    let enc_idx = props.encounter_idx;

    // Load available abilities when source changes
    use_effect(move || {
        let source = tracked_source.read().clone();

        selected_anchor.set(None);
        rotation.set(None);
        rotation_active.set(false);

        let Some(source_name) = source else {
            available_abilities.set(Vec::new());
            return;
        };

        spawn(async move {
            // Fetch with a dummy anchor to get the abilities list
            let result = api::query_rotation(enc_idx, &source_name, 0, None).await;
            if let Some(analysis) = result {
                available_abilities.set(analysis.abilities);
            }
        });
    });

    // Query rotation when Create is clicked (rotation_active) or time_range changes
    use_effect(move || {
        let tr = tracked_time_range();
        let active = rotation_active();

        if !active {
            return;
        }
        let Some(anchor_id) = *selected_anchor.peek() else {
            return;
        };
        let Some(ref source_name) = *tracked_source.peek() else {
            return;
        };
        let source_name = source_name.clone();

        let tr_opt = if tr.start == 0.0 && tr.end == 0.0 {
            None
        } else {
            Some(tr)
        };

        loading.set(true);
        spawn(async move {
            let result =
                api::query_rotation(enc_idx, &source_name, anchor_id, tr_opt.as_ref()).await;
            rotation.set(result);
            loading.set(false);
        });
    });

    let abilities = available_abilities.read().clone();
    let source = props.selected_source.clone();

    rsx! {
        div { class: "rotation-view",
            // Controls row
            div { class: "rotation-controls",
                label { "Create Rotation Visualisation:" }
                select {
                    class: "rotation-anchor-select",
                    value: selected_anchor().map(|id| id.to_string()).unwrap_or_default(),
                    onchange: move |evt: Event<FormData>| {
                        let val = evt.value();
                        selected_anchor.set(val.parse::<i64>().ok());
                        rotation.set(None);
                        rotation_active.set(false);
                    },
                    option { value: "", "-- Select Ability --" }
                    for (id, name) in &abilities {
                        option {
                            key: "{id}",
                            value: "{id}",
                            "{name}"
                        }
                    }
                }
                button {
                    class: "btn btn-primary",
                    disabled: selected_anchor().is_none() || source.is_none() || loading(),
                    onclick: move |_| {
                        rotation_active.set(true);
                    },
                    if loading() { "Loading..." } else { "Create" }
                }
            }

            if source.is_none() {
                div { class: "rotation-placeholder",
                    "Select a player from the sidebar to view their rotation."
                }
            }

            // Rotation cycles
            if let Some(ref analysis) = rotation() {
                if analysis.cycles.is_empty() {
                    div { class: "rotation-placeholder",
                        "No rotation data found for the selected anchor ability."
                    }
                } else {
                    div { class: "rotation-cycles",
                        for (i, cycle) in analysis.cycles.iter().enumerate() {
                            div { class: "rotation-cycle",
                                key: "{i}",
                                // Per-cycle stats
                                div { class: "rotation-cycle-stats",
                                    if cycle.total_damage > 0.0 && cycle.duration_secs > 0.0 {
                                        span { class: "cycle-stat dps",
                                            span { class: "cycle-stat-label", "DPS " }
                                            "{format_number(cycle.total_damage / cycle.duration_secs as f64)}"
                                        }
                                    }
                                    if cycle.effective_heal > 0.0 && cycle.duration_secs > 0.0 {
                                        span { class: "cycle-stat hps",
                                            span { class: "cycle-stat-label", "EHPS " }
                                            "{format_number(cycle.effective_heal / cycle.duration_secs as f64)}"
                                        }
                                    }
                                    if cycle.hit_count > 0 {
                                        span { class: "cycle-stat crit",
                                            span { class: "cycle-stat-label", "Crit " }
                                            "{format_pct(cycle.crit_count, cycle.hit_count)}"
                                        }
                                    }
                                    span { class: "cycle-stat duration",
                                        "{cycle.duration_secs:.1}s"
                                    }
                                }
                                div { class: "rotation-slots",
                                    for (j, slot) in cycle.slots.iter().enumerate() {
                                        {
                                        let gcd_time = slot.gcd_ability.time_secs;
                                        rsx! {
                                        div { class: "gcd-slot", key: "{j}",
                                            // Off-GCD weaves stacked above (reversed: last weave nearest GCD)
                                            for (k, weave) in slot.off_gcd.iter().rev().enumerate() {
                                                {
                                                let weave_time = weave.time_secs;
                                                rsx! {
                                                div { title: "{weave.ability_name}",
                                                    oncontextmenu: move |e: MouseEvent| {
                                                        if props.on_range_change.is_some() {
                                                            e.prevent_default();
                                                            context_menu_pos.set(Some((e.client_coordinates().x, e.client_coordinates().y)));
                                                            context_menu_time.set(weave_time);
                                                        }
                                                    },
                                                    AbilityIcon {
                                                        key: "w{k}",
                                                        ability_id: weave.ability_id,
                                                        size: 28,
                                                        fallback: weave.ability_name.clone(),
                                                    }
                                                }
                                                }
                                                }
                                            }
                                            // GCD ability on bottom
                                            div { title: "{slot.gcd_ability.ability_name}",
                                                oncontextmenu: move |e: MouseEvent| {
                                                    if props.on_range_change.is_some() {
                                                        e.prevent_default();
                                                        context_menu_pos.set(Some((e.client_coordinates().x, e.client_coordinates().y)));
                                                        context_menu_time.set(gcd_time);
                                                    }
                                                },
                                                AbilityIcon {
                                                    ability_id: slot.gcd_ability.ability_id,
                                                    size: 40,
                                                    fallback: slot.gcd_ability.ability_name.clone(),
                                                }
                                            }
                                            // GCD gap timing
                                            match slot.gcd_gap {
                                                Some(gap) => rsx! { span { class: "gcd-gap-time", "{gap:.3}" } },
                                                None => rsx! { span { class: "gcd-gap-time", visibility: "hidden", "0.000" } },
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

            // Context menu for "Set as range start/end"
            if let Some((x, y)) = *context_menu_pos.read() {
                if props.on_range_change.is_some() {
                    div {
                        style: "position: fixed; top: 0; left: 0; width: 100%; height: 100%; z-index: 999;",
                        onclick: move |_| context_menu_pos.set(None),
                    }
                    div {
                        class: "log-context-menu",
                        style: "position: fixed; left: {x}px; top: {y}px; z-index: 1000;",
                        div {
                            class: "context-menu-item",
                            onclick: move |_| {
                                let t = *context_menu_time.read();
                                if let Some(ref handler) = props.on_range_change {
                                    handler.call(TimeRange::new(t, props.time_range.end));
                                }
                                context_menu_pos.set(None);
                            },
                            "Set as range start"
                        }
                        div {
                            class: "context-menu-item",
                            onclick: move |_| {
                                let t = *context_menu_time.read();
                                if let Some(ref handler) = props.on_range_change {
                                    handler.call(TimeRange::new(props.time_range.start, t));
                                }
                                context_menu_pos.set(None);
                            },
                            "Set as range end"
                        }
                    }
                }
            }
        }
    }
}

fn format_number(value: f64) -> String {
    if value >= 1_000_000.0 {
        format!("{:.2}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("{:.1}K", value / 1_000.0)
    } else {
        format!("{:.0}", value)
    }
}

fn format_pct(count: i64, total: i64) -> String {
    if total == 0 {
        return "0%".to_string();
    }
    format!("{:.1}%", count as f64 / total as f64 * 100.0)
}
