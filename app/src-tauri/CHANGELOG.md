# v2026.2.12

Extensive improvements to the data explorer. More boss definitions. Bug fixes.

Features / Improvements:

- Extended ability breakdown tables with activations, miss/def%, effective healing%, shield detection, and
  average hit/crit columns
- Damage Taken tab: summary panel with damage type breakdown (internal/elemental, kinetic/energy,
  force/tech, melee/ranged) and mitigation stats (avoided, shielded, absorbed)
- Damage Taken tab: attack type and damage type columns per ability
- Health tracking chart added to data explorer
- Totals row on all ability breakdown tables
- Improved rotation analysis using explicit off-GCD ability classification instead of timing heuristic (thank you Mari for data)
- Data explorer defaults to local player instead of top value
- Phase timeline and combat log time range selection (start/end)
- Tab-colored value columns in ability tables for better readability

Timers and Definitions:

- Zorn & Toth encounter now ends on Handler Murdock death
- Brontes Clock phase start/end detection is more robust (credit Wolfy)
- Added Dxun II Pursuit droid phases and DPS challenge (credit Wolfy)
- Add Lady Dominique Phases and 2 alerts (credit Wolfy)
- Added definition file for Orbital Core
- Master Blaster is now classified as a success when the Blaster cast effect is removed and at least 1 player survives
- Chief Zokar is now detected as a boss on CZ-198 master mode
- XR-53 lethal strike timer changed from 30 seconds -> 32 seconds
- XR-53 lethal strike timer now cancels on recovery protocol

Fixes:

- Respect time range filter in effect uptime calculations
- Respect time range in shield attribution queries
- Remove self-damage events from data explorer damage tab
- Properly parse threat drops
- Protect recent log files (48h) from cleanup instead of date-based cutoff
- Fix stale timer pipeline_delay argument
- Fix profile selection dropdowns getting out-of-sync
