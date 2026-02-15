# v2026.2.5

Timer overlays have been tested swapping between profiles, toggling parsing on/off mid combat and others. At
this point the overlays should be stable.

### New Features

- Added Raid Notes overlay with basic markdown syntax support
- Added Data Explorer view link from session page
- Added clear-filters button to combat log view
- Moved alacrity and latency settings to top of session page
- Removed UI max height/width restrictions

### Boss Definitions & Flashpoints

- Added Spirit of Vengeance Flashpoint (partial data)
- Added Objective Meridian Flashpoint (Imperial)
- Added Mutated Geonosian Queen
- Updated Eyeless IDs for all difficulties
- Trandoshan Squad should correctly detect wipes and successes on NiM

### Improvements

- Combat log now displays effect names/IDs instead of abilities when effect is present
- Effects and timers will now hot reload when they are edited from the UI

### Bug Fixes

- Fixed timers A UI enabled/disabled not reflecting timer status
- Fixed combat log display failing to show effect names

# v2026.2.4

Sorry for the frequent updates- trying very hard to ensure the application is 100% stable.
Wayland keyboard shortcuts may work now -try them out!

### Timers and Definitions

- Added The Eyeless to Ops Encounters with crush timer
- Added Dreadful Entity to TFB definitions
- Dxun timer updates:
  - Flare timers for Dxun encounters 2 & 3
  - Czerka Pursuit Droid timer
  - Updates to Apex timers
- Gods from the Machine phase timers (first three bosses (SOME))
- Added Styrak Kell dragon spine timers

### Combat Log Improvements

- The combat log can now be navigate with the Home/End and Page Up/Down Keys
- Added separate filter for miscellaneous events
- Added Damage Type column
- Split Player and NPCs into separate sections in target/source selection drop downs
- Added toggle to display the raw timestamp

### Bug Fixes

- Fixed issue causing timers, effects, and raid frames to not populate after profile switch
- Fixed Wayland hotkey listener session issues
- Fixed potential race condition in UI
- Fixed several UI formatting errors
- Fixed line number tracking when starting parsing session in-combat
- Fixed issue causing certain combat encounters to fail to upload to parsely
- Definition files now log errors when they fail to parse

# v2026.2.3

- Implemented fix for regression causing parser to freeze.
- Encounters will now time out 5 seconds after the local player receives the revive immunity buff
- Ravager/TfB encounters no longer show all bosses as wipes
- Fixed issue causing some bosses to be registered in trash fights prior to encounter
- Enemies will only appear on the HP overlays after they have taken damage.
- Removed Watchdog as a kill target in Lady Dom, causing wipes to be classified as success

- Added experimental Wayland Hotkey support
- Changes to overlay state via hotkeys will now be reflected in the UI

# v2026.2.2 Hotfix

- **Fixed issue causing timers not to appear for new overlay profiles and new users**
- Improved wipe detection logic
- Encounters ended by exiting to med center will no longer appear to be in the area exited to
- Parsely upload success toast notification now stays until closed by user

# v2026.2.1

## What's New

### General

- Individual combats can now be uploaded to parsely.io via the session page
- Users can now set visibility and add an optional note when uploading to Parsely
- Starting the application in the middle of combat will now detect and parse the in-progress encounter
- UI positions and open elements are now preserved across tab-navigation; including the combat log scroll position
- Tweaked combat log formatting
- Improved handling of SWTOR combat log rotation upon character login/logout

### Encounter Classification

- Fake combat encounters that occur shortly after fights (e.g. Dread Master Holocrons) are now automatically ignored
- Fixed several edge cases causing encounter to split if mechanics are pushed too fast or player was revived at a specific time
- Fixed issue causing encounter to be classified as wipe if the local player used area start revive
- Coratanni boss fight will no longer appear split across multiple encounters if the local player dies during the encounter

### Timers and Bosses

- Fixed typo causing Ravagers default definitions failing to appear
- Fixed several text alerts on ToS firing on non-local player
