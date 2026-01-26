---
phase: quick-001
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - core/src/effects/definition.rs
  - core/src/timers/definition.rs
  - app/src-tauri/src/overlay/types.rs
  - types/src/lib.rs
  - overlay/src/overlays/mod.rs
  - app/src-tauri/src/overlay/spawn.rs
  - app/src-tauri/src/overlay/manager.rs
  - app/src-tauri/src/service/mod.rs
  - app/src/types.rs
  - app/src/components/encounter_editor/timers.rs
  - app/src/components/settings_panel.rs
autonomous: true

must_haves:
  truths:
    - "Encounter timers can be routed to Timers A or Timers B overlays via display_target field"
    - "Timers A is the default display target (backward compatible - current behavior)"
    - "Timers B exists with identical configuration options as Timers A"
    - "Users can select display target (Timers A or Timers B) in timer editor UI"
  artifacts:
    - path: "core/src/timers/definition.rs"
      provides: "TimerDisplayTarget enum with TimersA (default), TimersB, None variants"
    - path: "app/src-tauri/src/overlay/types.rs"
      provides: "OverlayType::TimersA and OverlayType::TimersB variants"
    - path: "types/src/lib.rs"
      provides: "TimersAConfig and TimersBConfig (reusing same struct, type aliases)"
  key_links:
    - from: "core/src/timers/definition.rs"
      to: "service timer routing"
      via: "display_target field on TimerDefinition"
    - from: "app/src-tauri/src/overlay/spawn.rs"
      to: "overlay creation"
      via: "create_timers_a_overlay and create_timers_b_overlay functions"
---

<objective>
Refactor encounter timer overlays to support A/B display targets, following the existing Effects A/B pattern.

Purpose: Allow users to route encounter timers to two separate overlay windows (Timers A and Timers B), enabling more flexible timer organization during encounters.

Output: TimerDefinition with display_target field, two timer overlays (A default, B optional), UI controls for selecting display target.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@core/src/effects/definition.rs (DisplayTarget enum pattern)
@core/src/timers/definition.rs (current TimerDefinition)
@app/src-tauri/src/overlay/types.rs (OverlayType enum)
@overlay/src/overlays/timers.rs (TimerOverlay implementation - reuse as-is)
@app/src-tauri/src/overlay/spawn.rs (overlay factory functions)
@types/src/lib.rs (config types)
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add TimerDisplayTarget enum and update TimerDefinition</name>
  <files>
    - core/src/effects/definition.rs
    - core/src/timers/definition.rs
  </files>
  <action>
1. In `core/src/timers/definition.rs`, add a new enum:
```rust
/// Which overlay should display this timer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerDisplayTarget {
    /// Show on Timers A overlay (default for backward compatibility)
    #[default]
    TimersA,
    /// Show on Timers B overlay
    TimersB,
    /// No overlay display (alerts only)
    None,
}
```

2. Add `display_target` field to `TimerDefinition` struct:
```rust
/// Which overlay should display this timer (defaults to TimersA)
#[serde(default)]
pub display_target: TimerDisplayTarget,
```

Place this in the Display section (after `show_on_raid_frames`).

3. Do NOT modify DisplayTarget in effects/definition.rs - timers use their own TimerDisplayTarget enum to keep concerns separate.
  </action>
  <verify>
    - `cargo check -p baras-core` compiles without errors
    - TimerDisplayTarget enum exists with TimersA, TimersB, None variants
    - TimerDefinition has display_target field defaulting to TimersA
  </verify>
  <done>
    - TimerDefinition supports display_target field
    - Existing timer TOML files without display_target will default to TimersA (backward compatible)
  </done>
</task>

<task type="auto">
  <name>Task 2: Add TimersA/TimersB overlay types and config</name>
  <files>
    - app/src-tauri/src/overlay/types.rs
    - types/src/lib.rs
    - overlay/src/overlays/mod.rs
    - app/src-tauri/src/overlay/spawn.rs
  </files>
  <action>
1. In `app/src-tauri/src/overlay/types.rs`, update OverlayType enum:
   - Rename `Timers` to `TimersA` (keeping "baras-timers" namespace for backward compat)
   - Add `TimersB` variant with namespace "baras-timers-b"
   - Update config_key(): "timers" -> "timers_a" for TimersA, "timers_b" for TimersB
   - Update namespace(): TimersA keeps "baras-timers", TimersB gets "baras-timers-b"
   - Update default_position(): TimersA keeps (650, 550), TimersB gets (650, 700)

2. In `types/src/lib.rs`:
   - Add type aliases for clarity (optional but recommended):
     ```rust
     /// Configuration for Timers A overlay (identical to TimerOverlayConfig)
     pub type TimersAConfig = TimerOverlayConfig;
     /// Configuration for Timers B overlay (identical to TimerOverlayConfig)
     pub type TimersBConfig = TimerOverlayConfig;
     ```
   - In OverlaySettings struct, add:
     ```rust
     #[serde(default)]
     pub timers_b_overlay: TimerOverlayConfig,
     #[serde(default = "default_opacity")]
     pub timers_b_opacity: u8,
     ```
   - Rename existing `timer_overlay` to `timers_a_overlay` with serde alias for backward compat:
     ```rust
     #[serde(default, alias = "timer_overlay")]
     pub timers_a_overlay: TimerOverlayConfig,
     #[serde(default = "default_opacity", alias = "timer_opacity")]
     pub timers_a_opacity: u8,
     ```

3. In `overlay/src/overlays/mod.rs`:
   - Update OverlayData enum: rename `Timers` to `TimersA`, add `TimersB(TimerData)`
   - Update OverlayConfigUpdate enum: rename `Timers` to `TimersA`, add `TimersB(TimerOverlayConfig, u8)`

4. In `app/src-tauri/src/overlay/spawn.rs`:
   - Rename `create_timer_overlay` to `create_timers_a_overlay`
   - Add `create_timers_b_overlay` (copy of create_timers_a_overlay with different namespace/kind)
   - TimersA uses namespace "baras-timers" and kind OverlayType::TimersA
   - TimersB uses namespace "baras-timers-b" and kind OverlayType::TimersB
   - Both use the same TimerOverlay struct from overlay crate (no changes needed there)
  </action>
  <verify>
    - `cargo check -p baras-overlay` compiles
    - `cargo check -p baras-types` compiles
    - `cargo check -p baras-app` compiles (src-tauri)
    - OverlayType has both TimersA and TimersB variants
  </verify>
  <done>
    - Two timer overlay types exist (TimersA, TimersB)
    - Both share the same underlying TimerOverlay implementation
    - Config supports both overlays with backward-compatible aliases
  </done>
</task>

<task type="auto">
  <name>Task 3: Update service routing, overlay manager, and UI</name>
  <files>
    - app/src-tauri/src/overlay/manager.rs
    - app/src-tauri/src/service/mod.rs
    - app/src/types.rs
    - app/src/components/encounter_editor/timers.rs
    - app/src/components/settings_panel.rs
  </files>
  <action>
1. In `app/src-tauri/src/overlay/manager.rs`:
   - Update spawn logic to handle OverlayType::TimersA and OverlayType::TimersB
   - Use create_timers_a_overlay for TimersA, create_timers_b_overlay for TimersB
   - Update any match arms on OverlayType that reference Timers

2. In `app/src-tauri/src/service/mod.rs`:
   - Update timer data routing to check display_target and send to appropriate overlay
   - Split timer entries by display_target when building TimerData
   - TimersA receives timers with display_target = TimersA (or legacy timers without field)
   - TimersB receives timers with display_target = TimersB
   - Update handler.rs if timer-related send_to_overlay calls exist

3. In `app/src/types.rs`:
   - Add TimerDisplayTarget enum (mirror of core enum for frontend):
     ```rust
     #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
     #[serde(rename_all = "snake_case")]
     pub enum TimerDisplayTarget {
         #[default]
         TimersA,
         TimersB,
         None,
     }

     impl TimerDisplayTarget {
         pub fn label(&self) -> &'static str {
             match self {
                 Self::TimersA => "Timers A",
                 Self::TimersB => "Timers B",
                 Self::None => "None",
             }
         }

         pub fn all() -> &'static [TimerDisplayTarget] {
             &[Self::TimersA, Self::TimersB, Self::None]
         }
     }
     ```
   - Update BossTimerDefinition struct to include display_target field

4. In `app/src/components/encounter_editor/timers.rs`:
   - Add display_target field to default_timer() function (default: TimerDisplayTarget::TimersA)
   - In TimerEditForm, add a dropdown for display_target selection in the Display section:
     ```rust
     div { class: "form-row-hz",
         label { "Display Target" }
         select {
             class: "select-inline",
             value: "{draft().display_target.label()}",
             onchange: move |e| {
                 let mut d = draft();
                 d.display_target = match e.value().as_str() {
                     "Timers A" => TimerDisplayTarget::TimersA,
                     "Timers B" => TimerDisplayTarget::TimersB,
                     "None" => TimerDisplayTarget::None,
                     _ => d.display_target,
                 };
                 draft.set(d);
             },
             for target in TimerDisplayTarget::all() {
                 option { value: "{target.label()}", "{target.label()}" }
             }
         }
     }
     ```

5. In `app/src/components/settings_panel.rs`:
   - Add Timers B overlay to overlay settings list (copy Timers A entry pattern)
   - Use timers_b_overlay and timers_b_opacity from settings
   - Keep Timers A as "Timers A" in UI (may already show as "Timers")
  </action>
  <verify>
    - `cargo build` compiles full workspace
    - Existing timer definitions load with display_target defaulting to TimersA
    - Timer editor shows display_target dropdown
    - Settings panel shows both Timers A and Timers B overlay options
  </verify>
  <done>
    - Timers route to correct overlay based on display_target
    - UI allows selecting Timers A, Timers B, or None for each timer
    - Both overlays configurable in settings panel
    - Backward compatible: existing configs work without changes
  </done>
</task>

</tasks>

<verification>
1. Build: `cargo build` succeeds
2. Load existing encounter with timers - all display on Timers A overlay
3. Edit a timer, change display_target to Timers B, save
4. Timer now appears on Timers B overlay (when enabled)
5. Settings panel shows both Timers A and Timers B with independent config
</verification>

<success_criteria>
- Encounter timers support display_target field (TimersA default, TimersB, None)
- Two independent timer overlays exist with shared TimerOverlay implementation
- Timer editor UI includes display target dropdown
- Settings panel includes Timers B overlay configuration
- Full backward compatibility: existing timers and configs work unchanged
</success_criteria>

<output>
After completion, create `.planning/quick/001-encounter-timers-a-b-display-targets/001-SUMMARY.md`
</output>
