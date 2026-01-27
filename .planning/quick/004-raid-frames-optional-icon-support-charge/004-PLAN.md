---
phase: quick
plan: 004
type: execute
wave: 1
depends_on: []
files_modified:
  - types/src/lib.rs
  - overlay/src/overlays/raid.rs
  - app/src/components/settings_panel.rs
autonomous: true

must_haves:
  truths:
    - "Raid frame effects can optionally display icons instead of colored squares"
    - "Icons show wipedown effect (top-down black overlay) based on remaining duration"
    - "Icons display charge count when charges > 1"
    - "Icons do NOT show countdown text (too small for raid frames)"
    - "Icon display is opt-in with default false"
  artifacts:
    - path: "types/src/lib.rs"
      provides: "show_effect_icons field in RaidOverlaySettings"
      contains: "show_effect_icons"
    - path: "overlay/src/overlays/raid.rs"
      provides: "Icon rendering with wipedown effect in render_effects"
      contains: "draw_image"
    - path: "app/src/components/settings_panel.rs"
      provides: "Toggle for show_effect_icons setting"
      contains: "show_effect_icons"
  key_links:
    - from: "overlay/src/overlays/raid.rs"
      to: "RaidEffect.icon"
      via: "conditional icon rendering in render_effects"
      pattern: "show_effect_icons.*icon"
---

<objective>
Enable optional icon rendering for raid frame effects with wipedown effect and charge display.

Purpose: Allow users to see ability icons on raid frame effects instead of colored squares, matching the visual style of effects_ab overlay but optimized for small display (no countdown text).

Output: Working icon display toggle for raid frames with proper wipedown animation and charge count.
</objective>

<execution_context>
@/home/prescott/.claude/get-shit-done/workflows/execute-plan.md
@/home/prescott/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@overlay/src/overlays/raid.rs
@overlay/src/overlays/effects_ab.rs (reference for icon rendering pattern)
@types/src/lib.rs (RaidOverlaySettings struct around line 915)
@app/src/components/settings_panel.rs (raid settings section)
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add show_effect_icons setting and icon field to RaidEffect</name>
  <files>types/src/lib.rs, overlay/src/overlays/raid.rs</files>
  <action>
1. In types/src/lib.rs, add to RaidOverlaySettings struct (around line 915):
   ```rust
   #[serde(default)]
   pub show_effect_icons: bool,
   ```
   Default is false (opt-in feature). Add it after effect_fill_opacity field.

2. In overlay/src/overlays/raid.rs, add icon field to RaidEffect struct:
   ```rust
   /// Pre-loaded icon RGBA data (width, height, rgba_bytes) - Arc for cheap cloning
   pub icon: Option<std::sync::Arc<(u32, u32, Vec<u8>)>>,
   ```
   Add after is_buff field. Initialize to None in new() method.

3. In overlay/src/overlays/raid.rs, add show_effect_icons to RaidOverlayConfig struct:
   ```rust
   /// Whether to render effect icons (true) or colored squares (false)
   pub show_effect_icons: bool,
   ```
   Default to false. Update Default impl and From<RaidOverlaySettings> impl.
  </action>
  <verify>cargo check -p baras-types && cargo check -p baras-overlay</verify>
  <done>RaidOverlaySettings has show_effect_icons field, RaidEffect has icon field, RaidOverlayConfig has show_effect_icons</done>
</task>

<task type="auto">
  <name>Task 2: Implement icon rendering with wipedown effect in raid overlay</name>
  <files>overlay/src/overlays/raid.rs</files>
  <action>
Modify render_effects method to conditionally render icons:

1. At the start of the for loop over effects, check if icon should be rendered:
   ```rust
   // Draw icon or colored square
   let has_icon = if self.config.show_effect_icons {
       if let Some(ref icon_arc) = effect.icon {
           let (img_w, img_h, ref rgba) = **icon_arc;
           self.frame.draw_image(rgba, img_w, img_h, ex, ey, effect_size, effect_size);
           true
       } else {
           false
       }
   } else {
       false
   };

   if !has_icon {
       // Existing colored square rendering (dark background + fill)
       // ... existing code ...
   }
   ```

2. After icon/square rendering, add wipedown overlay (from effects_ab pattern):
   ```rust
   // Wipedown overlay (always applied, works for both icon and colored square)
   let progress = effect.fill_percent();
   let overlay_height = effect_size * (1.0 - progress);
   if overlay_height > 1.0 {
       self.frame.fill_rect(
           ex,
           ey,
           effect_size,
           overlay_height,
           Color::from_rgba8(0, 0, 0, 140),
       );
   }
   ```

3. Keep the existing border and charge count rendering (charges > 1).
   The charge display already exists and centers the count text.

4. DO NOT add countdown text - raid frame effect squares are too small.
  </action>
  <verify>cargo check -p baras-overlay</verify>
  <done>render_effects conditionally renders icons with wipedown effect, charges still displayed, no countdown text</done>
</task>

<task type="auto">
  <name>Task 3: Add UI toggle for show_effect_icons setting</name>
  <files>app/src/components/settings_panel.rs</files>
  <action>
In the raid overlay settings section (search for "Raid Frames" or raid_overlay), add a toggle for show_effect_icons:

Find the existing raid settings toggles (like show_role_icons) and add a similar toggle:

```rust
// Effect Icons toggle
label { class: "toggle-row",
    input {
        r#type: "checkbox",
        checked: current_settings.raid_overlay.show_effect_icons,
        onchange: move |evt| {
            let mut new_settings = draft_settings();
            new_settings.raid_overlay.show_effect_icons = evt.checked();
            update_draft(new_settings);
        },
    }
    span { "Show Effect Icons" }
}
```

Add after the existing effect settings (effect_size, effect_vertical_offset, effect_fill_opacity sliders).
Add a hint explaining the feature:
```rust
p { class: "hint", "Display ability icons instead of colored squares (requires icon pack)" }
```
  </action>
  <verify>cargo check -p baras-app</verify>
  <done>Settings panel has toggle for show_effect_icons with hint text</done>
</task>

</tasks>

<verification>
1. cargo check -p baras-types -p baras-overlay -p baras-app
2. cargo clippy -p baras-overlay -- -D warnings (check for new warnings)
3. Verify RaidOverlaySettings has show_effect_icons defaulting to false
4. Verify RaidEffect has icon field
5. Verify render_effects handles both icon and colored square rendering
</verification>

<success_criteria>
- show_effect_icons setting exists in types, overlay config, and settings UI
- When enabled, raid frame effects render icons (if available) with wipedown effect
- When disabled (default), raid frame effects render colored squares (existing behavior)
- Charge count (> 1) displays on both icon and colored square modes
- No countdown text on raid frame effects (too small)
- No regression in existing raid frame functionality
</success_criteria>

<output>
After completion, create `.planning/quick/004-raid-frames-optional-icon-support-charge/004-SUMMARY.md`
</output>
