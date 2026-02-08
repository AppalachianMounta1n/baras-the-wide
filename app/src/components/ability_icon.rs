//! Reusable ability icon component
//!
//! Fetches and displays ability icons by ID with async loading and caching.

use std::cell::RefCell;
use std::collections::HashMap;

use dioxus::prelude::*;

use crate::api;

// Thread-local cache for icon URLs (WASM is single-threaded)
// Stores Some(url) for found icons, None for missing icons (to avoid re-fetching)
thread_local! {
    static ICON_CACHE: RefCell<HashMap<u64, Option<String>>> = RefCell::new(HashMap::new());
}

/// Check cache for an icon URL
fn get_cached(ability_id: u64) -> Option<Option<String>> {
    ICON_CACHE.with(|cache| cache.borrow().get(&ability_id).cloned())
}

/// Store an icon URL in cache
fn set_cached(ability_id: u64, url: Option<String>) {
    ICON_CACHE.with(|cache| {
        cache.borrow_mut().insert(ability_id, url);
    });
}

/// Ability icon component that fetches and displays an icon by ability ID.
///
/// Uses a global cache to prevent redundant API calls when scrolling
/// through lists with repeated abilities.
#[component]
pub fn AbilityIcon(
    ability_id: i64,
    #[props(default = 20)] size: u32,
    /// Fallback text shown when icon is missing (e.g., ability name initials).
    #[props(default = String::new())]
    fallback: String,
) -> Element {
    let mut icon_url = use_signal(|| None::<String>);
    let mut loaded = use_signal(|| false);
    let id = ability_id as u64;

    use_effect(move || {
        // Check cache first
        if let Some(cached) = get_cached(id) {
            icon_url.set(cached);
            loaded.set(true);
            return;
        }

        // Not in cache - fetch and store
        spawn(async move {
            let result = api::get_icon_preview(id).await;
            set_cached(id, result.clone());
            icon_url.set(result);
            loaded.set(true);
        });
    });

    rsx! {
        if let Some(ref url) = icon_url() {
            img {
                src: "{url}",
                class: "ability-icon",
                width: "{size}",
                height: "{size}",
                alt: ""
            }
        } else if loaded() && !fallback.is_empty() {
            div {
                class: "ability-icon-fallback",
                width: "{size}px",
                height: "{size}px",
                title: "{fallback}",
                "{fallback_initials(&fallback)}"
            }
        }
    }
}

/// Extract up to 2 uppercase initials from ability name for fallback display.
fn fallback_initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}
