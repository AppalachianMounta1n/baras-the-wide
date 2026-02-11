# v2026

Features / Improvements:

- Extended ability breakdown tables with activations, miss/def%, effective healing%, shield detection, and
  average hit/crit columns
- Damage Taken tab: summary panel with damage type breakdown (internal/elemental, kinetic/energy,
  force/tech, melee/ranged) and mitigation stats (avoided, shielded, absorbed)
- Damage Taken tab: attack type and damage type columns per ability
- Totals row on all ability breakdown tables
- Improved rotation analysis using explicit off-GCD ability classification instead of timing heuristic
- Data explorer defaults to local player instead of top value
- Phase timeline and combat log time range selection (start/end)
- Tab-colored value columns in ability tables for better readability
- Data explorer tabs now default to local player

Fixes:

- Respect time range filter in effect uptime calculations
- Respect time range in shield attribution queries
- Remove self-damage events from data explorer
- Properly parse threat drops
- Protect recent log files (48h) from cleanup instead of date-based cutoff
- Fix stale timer pipeline_delay argument
