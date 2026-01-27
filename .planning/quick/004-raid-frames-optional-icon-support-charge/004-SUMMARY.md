---
phase: quick
plan: 004
subsystem: overlay
tags: [raid-frames, effects, icons, UI]

dependency-graph:
  requires: []
  provides: [raid-frame-icon-effects, effect-wipedown-animation]
  affects: []

tech-stack:
  added: []
  patterns: [conditional-rendering, icon-arc-sharing]

key-files:
  created: []
  modified:
    - types/src/lib.rs
    - overlay/src/overlays/raid.rs
    - app/src/components/settings_panel.rs

decisions:
  - id: Q004-01
    choice: "Wipedown for both modes"
    reason: "Apply wipedown overlay to both icons and colored squares for consistent visual feedback"

metrics:
  duration: "3 min"
  completed: 2026-01-27
---

# Quick Task 004: Raid Frames Optional Icon Support (Charge) Summary

Optional icon rendering for raid frame effects with wipedown animation and charge display.

## What Was Built

### Types Layer (types/src/lib.rs)
- Added `show_effect_icons: bool` field to `RaidOverlaySettings` with `#[serde(default)]`
- Default value: `false` (opt-in feature)
- Updated `Default` impl to include the new field

### Overlay Layer (overlay/src/overlays/raid.rs)
- Added `icon: Option<Arc<(u32, u32, Vec<u8>)>>` field to `RaidEffect` struct for pre-loaded icon data
- Added `show_effect_icons: bool` field to `RaidOverlayConfig`
- Updated `Default` and `From<RaidOverlaySettings>` impls
- Modified `render_effects` method:
  - Conditionally renders icons when `show_effect_icons` enabled and icon data available
  - Falls back to colored square rendering when no icon
  - Added wipedown overlay (darkened top area) based on remaining duration
  - Wipedown works for both icon and colored square modes
  - Kept existing border and charge count (> 1) rendering

### App Layer (app/src/components/settings_panel.rs)
- Added "Show Effect Icons" checkbox toggle in Raid Frames settings section
- Added hint text: "Display ability icons instead of colored squares (requires icon pack)"

## Decisions Made

| ID | Decision | Rationale |
|----|----------|-----------|
| Q004-01 | Wipedown for both modes | Apply wipedown overlay to both icons and colored squares for consistent visual feedback on duration remaining |

## Deviations from Plan

None - plan executed exactly as written.

## Commits

| Hash | Type | Description |
|------|------|-------------|
| 407377d | feat | Add show_effect_icons setting and icon field |
| 01cb6e7 | feat | Implement icon rendering with wipedown in raid overlay |
| e6b29ff | feat | Add UI toggle for show_effect_icons setting |

## Verification Results

- `cargo check -p baras-types -p baras-overlay` - PASS
- `cargo check --manifest-path app/src-tauri/Cargo.toml` - PASS
- RaidOverlaySettings has show_effect_icons defaulting to false - VERIFIED
- RaidEffect has icon field - VERIFIED
- render_effects handles both icon and colored square rendering - VERIFIED

## Notes

- Icon loading is handled by the service layer (not part of this task)
- The `RaidEffect.icon` field uses `Arc<(u32, u32, Vec<u8>)>` for efficient sharing of icon data across effects
- No countdown text on raid frame effects (too small) - charge count only
- Wipedown effect matches the effects_ab overlay visual style
