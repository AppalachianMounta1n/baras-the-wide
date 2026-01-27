---
phase: quick-005
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - app/src/components/effect_editor.rs
  - app/src/components/encounter_editor/timers.rs
  - app/src/components/encounter_editor/phases.rs
  - app/src/components/encounter_editor/counters.rs
  - app/src/components/encounter_editor/challenges.rs
  - app/src/components/encounter_editor/entities.rs
  - app/assets/main.css
autonomous: true

must_haves:
  truths:
    - "User sees visual indicator when an expanded item has unsaved changes"
    - "Indicator appears on row header when form is dirty"
    - "Indicator disappears after saving"
  artifacts:
    - path: "app/assets/main.css"
      provides: "CSS for unsaved indicator styling"
      contains: ".unsaved-indicator"
  key_links:
    - from: "effect_editor.rs EffectRow"
      to: "has_changes memo"
      via: "conditional class or element"
    - from: "encounter_editor timers.rs TimerRow"
      to: "has_changes memo"
      via: "conditional class or element"
---

<objective>
Add unsaved changes indicator to encounter editor and effects tracker editor

Purpose: Users need visual feedback when an expanded item has been modified but not yet saved. Currently the only indication is that the Save button becomes enabled - but this is easy to miss, leading to accidental data loss when collapsing without saving.

Output: Visual indicator (dot/badge) on row headers when draft differs from original
</objective>

<execution_context>
@/home/prescott/.claude/get-shit-done/workflows/execute-plan.md
@/home/prescott/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@app/src/components/effect_editor.rs
@app/src/components/encounter_editor/timers.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add CSS for unsaved indicator</name>
  <files>app/assets/main.css</files>
  <action>
Add CSS class for the unsaved changes indicator. Style it as a small orange/yellow dot or asterisk that appears inline with the item name.

```css
/* Unsaved changes indicator */
.unsaved-indicator {
  display: inline-block;
  width: 8px;
  height: 8px;
  background: var(--color-warning, #f0ad4e);
  border-radius: 50%;
  margin-left: 6px;
  animation: pulse-subtle 2s ease-in-out infinite;
}

@keyframes pulse-subtle {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.6; }
}
```

Place this near other editor-related styles (search for `.effect-editor` or `.list-item` sections).
  </action>
  <verify>CSS file contains `.unsaved-indicator` class with styling</verify>
  <done>Unsaved indicator CSS class exists and provides visual styling</done>
</task>

<task type="auto">
  <name>Task 2: Add indicator to Effect Editor rows</name>
  <files>app/src/components/effect_editor.rs</files>
  <action>
Modify `EffectRow` component to pass `has_changes` state up and display indicator.

The challenge: `has_changes` is computed inside `EffectEditForm`, but we need to show the indicator in `EffectRow` header (which wraps the form).

Solution: Lift `has_changes` computation to `EffectRow`:

1. In `EffectRow`, add a signal to track the draft state:
   ```rust
   let mut draft_state = use_signal(|| effect.clone());
   ```

2. Compute has_changes at EffectRow level:
   ```rust
   let effect_original = effect.clone();
   let has_changes = use_memo(move || expanded && draft_state() != effect_original);
   ```

3. Add indicator in the row header (after the effect name span, around line 542):
   ```rust
   span { class: "effect-name", "{effect.name}" }
   if has_changes() {
       span { class: "unsaved-indicator", title: "Unsaved changes" }
   }
   ```

4. Pass `draft_state` to `EffectEditForm` and have the form update it via an `on_draft_change` callback, OR simpler approach: have `EffectEditForm` accept an `on_dirty` callback that it calls when changes occur.

SIMPLER APPROACH: Since `EffectRow` already passes `effect` to `EffectEditForm`, and the form maintains its own draft, we can:
- Add a `draft_dirty` signal to `EffectRow`
- Pass `on_dirty: EventHandler<bool>` to `EffectEditForm`
- Have `EffectEditForm` call `on_dirty(true/false)` whenever its internal `has_changes` memo changes (via use_effect)

In EffectRow (around line 517):
```rust
let mut is_dirty = use_signal(|| false);
```

In the header (after effect name):
```rust
if expanded && is_dirty() {
    span { class: "unsaved-indicator", title: "Unsaved changes" }
}
```

Pass to EffectEditForm:
```rust
EffectEditForm {
    effect: effect.clone(),
    is_draft: is_draft,
    on_save: on_save,
    on_delete: on_delete,
    on_duplicate: on_duplicate,
    on_dirty: move |dirty: bool| is_dirty.set(dirty),
}
```

In EffectEditForm, add the prop and effect:
```rust
#[props(default)] on_dirty: EventHandler<bool>,
```

And a use_effect to sync:
```rust
use_effect(move || {
    on_dirty.call(has_changes());
});
```
  </action>
  <verify>Cargo check passes for app crate</verify>
  <done>Effect editor rows show orange dot when expanded item has unsaved changes</done>
</task>

<task type="auto">
  <name>Task 3: Add indicator to Encounter Editor rows</name>
  <files>
    app/src/components/encounter_editor/timers.rs
    app/src/components/encounter_editor/phases.rs
    app/src/components/encounter_editor/counters.rs
    app/src/components/encounter_editor/challenges.rs
    app/src/components/encounter_editor/entities.rs
  </files>
  <action>
Apply the same pattern to all encounter editor tab files. Each has a similar structure:
- `*Tab` component with list of items
- `*Row` component for individual items
- `*EditForm` component for the expanded edit form

For each file (timers.rs, phases.rs, counters.rs, challenges.rs, entities.rs):

1. In the Row component (e.g., `TimerRow`), add dirty tracking signal:
   ```rust
   let mut is_dirty = use_signal(|| false);
   ```

2. In the row header, after the item name, add indicator:
   ```rust
   span { class: "font-medium text-primary truncate", "{timer.name}" }
   if expanded && is_dirty() {
       span { class: "unsaved-indicator", title: "Unsaved changes" }
   }
   ```

3. Pass `on_dirty` to the EditForm:
   ```rust
   TimerEditForm {
       // ... existing props ...
       on_dirty: move |dirty: bool| is_dirty.set(dirty),
   }
   ```

4. In the EditForm component, add the prop:
   ```rust
   #[props(default)] on_dirty: EventHandler<bool>,
   ```

5. Add use_effect to sync dirty state:
   ```rust
   use_effect(move || {
       on_dirty.call(has_changes());
   });
   ```

Repeat for all 5 tab files. The pattern is identical - just the type names differ (Timer, Phase, Counter, Challenge, Entity).
  </action>
  <verify>Cargo check passes for app crate</verify>
  <done>All encounter editor item rows show orange dot when expanded and modified</done>
</task>

</tasks>

<verification>
1. `cargo check -p app` passes
2. Run app and expand an effect in Effect Editor
3. Modify any field - orange dot should appear next to effect name
4. Click Save - dot should disappear
5. Repeat for encounter editor: expand a timer, modify it, verify dot appears
6. Test phases, counters, challenges, entities tabs similarly
</verification>

<success_criteria>
- Orange pulsing dot appears on row header when item has unsaved changes
- Indicator only shows when item is expanded AND has changes
- Indicator disappears after saving
- Works in both Effect Editor and all Encounter Editor tabs
- No compilation errors
</success_criteria>

<output>
After completion, create `.planning/quick/005-unsaved-changes-indicator-editors/005-SUMMARY.md`
</output>
