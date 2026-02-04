//! Encounter history tracking and classification
//!
//! Provides persistence of encounter metrics across the current log file session,
//! classification of encounters by phase type, and human-readable naming.

use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

use super::CombatEncounter;
use super::PhaseType;
use super::entity_info::PlayerInfo;
use super::metrics::PlayerMetrics;
use crate::combat_log::EntityType;
use crate::context::resolve;
use crate::debug_log;
use crate::game_data::{BossInfo, ContentType, Difficulty, is_pvp_area, lookup_boss};
use crate::state::info::AreaInfo;

/// Summary of a completed encounter with computed metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncounterSummary {
    pub encounter_id: u64,
    pub display_name: String,
    pub encounter_type: PhaseType,
    /// ISO 8601 formatted start time (or None if unknown)
    pub start_time: Option<String>,
    /// ISO 8601 formatted end time (or None if unknown)
    pub end_time: Option<String>,
    pub duration_seconds: i64,
    pub success: bool,
    pub area_name: String,
    pub difficulty: Option<String>,
    pub boss_name: Option<String>,
    pub player_metrics: Vec<PlayerMetrics>,
    /// True if this encounter starts a new phase (area change)
    pub is_phase_start: bool,
    /// Names of NPC enemies in the encounter
    pub npc_names: Vec<String>,

    // ─── Line Number Tracking (for per-encounter Parsely uploads) ────────────
    /// Line number of the most recent AreaEntered event before this encounter
    pub area_entered_line: Option<u64>,
    /// First line of events for this encounter (after previous encounter ended)
    pub event_start_line: Option<u64>,
    /// Last line of events for this encounter (includes grace period)
    pub event_end_line: Option<u64>,

    // ─── Parsely Integration ─────────────────────────────────────────────────
    /// Link to the uploaded encounter on Parsely (set after successful upload)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parsely_link: Option<String>,
}

/// Tracks encounter history for the current log file session
#[derive(Debug, Clone, Default)]
pub struct EncounterHistory {
    summaries: Vec<EncounterSummary>,
    boss_pull_counts: HashMap<String, u32>,
    trash_pull_count: u32,
    /// Generation counter from AreaInfo, used to detect phase boundaries
    /// (including re-entering the same area).
    current_generation: Option<u64>,
}

impl EncounterHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, summary: EncounterSummary) {
        self.summaries.push(summary);
    }

    pub fn summaries(&self) -> &[EncounterSummary] {
        &self.summaries
    }

    /// Update the parsely_link for an encounter by its ID
    pub fn set_parsely_link(&mut self, encounter_id: u64, link: String) -> bool {
        if let Some(summary) = self.summaries.iter_mut().find(|s| s.encounter_id == encounter_id) {
            summary.parsely_link = Some(link);
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        self.summaries.clear();
        self.boss_pull_counts.clear();
        self.trash_pull_count = 0;
        self.current_generation = None;
    }

    /// Check if area changed and update tracking.
    /// Uses the area generation counter so re-entering the same area
    /// (e.g., running the same flashpoint twice) is detected as a new phase.
    pub fn check_area_change(&mut self, generation: u64) -> bool {
        let changed = self.current_generation != Some(generation);
        if changed {
            self.current_generation = Some(generation);
            // Reset pull counts on area change
            self.trash_pull_count = 0;
            self.boss_pull_counts.clear();
        }
        changed
    }
    
    /// Restore the generation counter without resetting pull counts.
    /// Used when importing encounter history from subprocess - the pull counts
    /// are already reflected in the imported encounter names.
    pub fn restore_generation(&mut self, generation: u64) {
        self.current_generation = Some(generation);
    }
    
    /// Rebuild pull counts from imported encounter summaries.
    /// This extracts the counts from encounter names like "Boss - 3" or "Raid Trash 5".
    /// Call this after importing encounters to ensure live encounters continue with correct numbering.
    /// 
    /// Only counts encounters from the CURRENT phase (after the last is_phase_start).
    /// When you enter a new area, pull counts reset, so we shouldn't carry over counts from previous areas.
    pub fn rebuild_pull_counts(&mut self) {
        self.trash_pull_count = 0;
        self.boss_pull_counts.clear();
        
        // Find the index of the last phase start (area change)
        let last_phase_start_idx = self.summaries.iter()
            .enumerate()
            .rev()
            .find(|(_, s)| s.is_phase_start)
            .map(|(idx, _)| idx);
        
        // Only count encounters from the current phase onwards
        let encounters_to_count = if let Some(start_idx) = last_phase_start_idx {
            &self.summaries[start_idx..]
        } else {
            &self.summaries[..]
        };
        
        for summary in encounters_to_count {
            if let Some(ref boss_name) = summary.boss_name {
                // Extract count from "Boss Name - N" format
                if let Some(count_str) = summary.display_name.rsplit(" - ").next() {
                    if let Ok(count) = count_str.parse::<u32>() {
                        let current = self.boss_pull_counts.get(boss_name).copied().unwrap_or(0);
                        self.boss_pull_counts.insert(boss_name.clone(), current.max(count));
                    }
                }
            } else {
                // Trash encounter - extract count from "Type N" format
                if let Some(count_str) = summary.display_name.rsplit(' ').next() {
                    if let Ok(count) = count_str.parse::<u32>() {
                        self.trash_pull_count = self.trash_pull_count.max(count);
                    }
                }
            }
        }
    }

    /// Generate a human-readable name for an encounter based on its type and boss
    pub fn generate_name(&mut self, encounter_type: PhaseType, boss_name: Option<&str>) -> String {
        match (encounter_type, boss_name) {
            // Boss encounter: "Brontes - 7"
            (_, Some(name)) => {
                let count = self.boss_pull_counts.entry(name.to_string()).or_insert(0);
                *count += 1;
                format!("{} - {}", name, count)
            }
            (PhaseType::Raid, None) => {
                self.trash_pull_count += 1;
                format!("Raid Trash {}", self.trash_pull_count)
            }
            (PhaseType::Flashpoint, None) => {
                self.trash_pull_count += 1;
                format!("Flashpoint Trash {}", self.trash_pull_count)
            }
            (PhaseType::DummyParse, None) => {
                self.trash_pull_count += 1;
                format!("Dummy Parse {}", self.trash_pull_count)
            }
            (PhaseType::PvP, None) => {
                self.trash_pull_count += 1;
                format!("PvP Match {}", self.trash_pull_count)
            }
            (PhaseType::OpenWorld, None) => {
                self.trash_pull_count += 1;
                format!("Open World {}", self.trash_pull_count)
            }
        }
    }

    /// Peek the current pull count for a boss without incrementing.
    /// Returns what the pull number would be for an in-progress encounter.
    /// Used for live overlay display before the encounter is finalized.
    pub fn peek_pull_count(&self, boss_name: &str) -> u32 {
        self.boss_pull_counts.get(boss_name).copied().unwrap_or(0) + 1
    }

    /// Peek the current trash pull count without incrementing.
    pub fn peek_trash_count(&self) -> u32 {
        self.trash_pull_count + 1
    }
}

/// Classify an encounter's phase type and find the primary boss (if any)
/// Uses difficulty ID for phase classification, with training dummy override
pub fn classify_encounter(
    encounter: &CombatEncounter,
    area_id: i64,
    difficulty_id: i64,
) -> (PhaseType, Option<&'static BossInfo>) {
    // 1. Find boss info if present (sorted by first_seen_at for primary boss)
    let boss_info = if encounter.npcs.values().any(|v| v.is_boss) {
        let mut boss_npcs: Vec<_> = encounter
            .npcs
            .values()
            .filter_map(|npc| lookup_boss(npc.class_id).map(|info| (npc, info)))
            .collect();
        boss_npcs.sort_by_key(|(npc, _)| npc.first_seen_at);
        boss_npcs.first().map(|(_, info)| *info)
    } else {
        None
    };

    // 2. Check for training dummy (overrides all other classification)
    if let Some(info) = boss_info
        && info.content_type == ContentType::TrainingDummy
    {
        return (PhaseType::DummyParse, Some(info));
    }
    if let Some(def) = encounter.active_boss_definition()
        && def.area_type == crate::dsl::AreaType::TrainingDummy
    {
        return (PhaseType::DummyParse, boss_info);
    }

    // 3. Check PvP area
    if is_pvp_area(area_id) {
        return (PhaseType::PvP, boss_info);
    }

    // 4. Classify by difficulty ID
    let phase = if let Some(difficulty) = Difficulty::from_difficulty_id(difficulty_id) {
        match difficulty.group_size() {
            8 | 16 => PhaseType::Raid,
            4 => PhaseType::Flashpoint,
            _ => PhaseType::OpenWorld,
        }
    } else {
        PhaseType::OpenWorld
    };

    (phase, boss_info)
}

/// Determine if an encounter was successful (not a wipe)
/// Returns false (wipe) if all players died or kill targets are still alive
/// For victory-trigger encounters, success requires the trigger to have fired
/// (but only if the victory trigger applies to the current difficulty)
pub fn determine_success(encounter: &CombatEncounter) -> bool {
    // Check if the ACTIVE boss requires a victory trigger
    // Only check the specific boss being fought, not all bosses in the area
    if let Some(idx) = encounter.active_boss_idx() {
        let def = &encounter.boss_definitions()[idx];
        if def.has_victory_trigger {
            // Check if victory trigger applies to current difficulty
            // If victory_trigger_difficulties is empty, it applies to all
            // If specified, only require the trigger on matching difficulties
            let trigger_applies = if def.victory_trigger_difficulties.is_empty() {
                true
            } else {
                encounter
                    .difficulty
                    .as_ref()
                    .map(|d| {
                        def.victory_trigger_difficulties
                            .iter()
                            .any(|vd| d.matches_config_key(vd))
                    })
                    .unwrap_or(false) // If difficulty unknown, don't require trigger
            };

            if trigger_applies {
                // Hard requirement: victory trigger must have fired for success
                // If the trigger never fired, it's always a wipe
                return encounter.victory_triggered;
            }
            // Victory trigger doesn't apply to this difficulty, fall through to normal logic
        }
    }
    
    // Standard encounters: use is_likely_wipe()
    !encounter.is_likely_wipe()
}

/// Create an EncounterSummary from a completed CombatEncounter
pub fn create_encounter_summary(
    encounter: &CombatEncounter,
    area: &AreaInfo,
    history: &mut EncounterHistory,
    player_disciplines: &HashMap<i64, PlayerInfo>,
) -> Option<EncounterSummary> {
    // Skip encounters that never started combat
    #[allow(clippy::question_mark)]
    if encounter.enter_combat_time.is_none() {
        return None;
    }

    // DEBUG: Log wipe detection state with player details
    let combat_start = encounter.enter_combat_time;
    let player_states: Vec<String> = encounter
        .players
        .values()
        .map(|p| {
            let in_combat =
                combat_start.is_none_or(|start| p.last_seen_at.is_some_and(|seen| seen >= start));
            format!(
                "{}:dead={},in_combat={}",
                resolve(p.name),
                p.is_dead,
                in_combat
            )
        })
        .collect();
    debug_log!(
        "create_encounter_summary: all_dead={}, players={}, states=[{}]",
        encounter.all_players_dead,
        encounter.players.len(),
        player_states.join(", ")
    );

    // Use encounter's stored area/difficulty info (falls back to cache if not set)
    // This ensures we use the area where the fight took place, not where player ended up
    let area_id_for_classification = encounter.area_id.unwrap_or(area.area_id);
    let difficulty_id_for_classification = encounter.difficulty_id.unwrap_or(area.difficulty_id);
    let encounter_area_name = encounter.area_name.clone().unwrap_or_else(|| area.area_name.clone());

    // Check if this is a new phase (area change)
    // Compare encounter's area_name with the last summary's area_name
    // This prevents creating new sections when player temporarily leaves and returns to same area
    let is_phase_start = history.summaries().last()
        .map(|last| last.area_name != encounter_area_name)
        .unwrap_or(true);  // First encounter is always a phase start
    
    // Reset pull counts on phase change (area transition)
    // This ensures encounter numbering restarts from 1 in each new area
    if is_phase_start {
        history.boss_pull_counts.clear();
        history.trash_pull_count = 0;
    }
    
    // Classify using encounter's area info
    let (encounter_type, boss_info) = classify_encounter(
        encounter,
        area_id_for_classification,
        difficulty_id_for_classification,
    );

    // Get boss name: prefer active definition, fall back to detected boss NPC
    // This allows non-boss trigger entities to classify the encounter
    let boss_name = encounter
        .active_boss_definition()
        .map(|def| def.name.clone())
        .or_else(|| {
            // Only fall back to hardcoded data if a boss NPC was actually seen
            if encounter.npcs.values().any(|v| v.is_boss) {
                boss_info.map(|b| b.boss.to_string())
            } else {
                None
            }
        });

    let display_name = history.generate_name(encounter_type, boss_name.as_deref());

    // Calculate metrics and filter to players seen during actual combat
    let combat_start = encounter.enter_combat_time;
    let player_metrics: Vec<PlayerMetrics> = encounter
        .calculate_entity_metrics(player_disciplines)
        .unwrap_or_default()
        .into_iter()
        .filter(|m| {
            // Filter out NPCs
            if m.entity_type == EntityType::Npc {
                return false;
            }
            // Filter out players not seen during combat (e.g., character switches)
            encounter.players.get(&m.entity_id).is_some_and(|p| {
                combat_start.is_none_or(|start| p.last_seen_at.is_some_and(|seen| seen >= start))
            })
        })
        .map(|m| m.to_player_metrics())
        .collect();

    // Use encounter's stored difficulty (falls back to cache if not set)
    let difficulty = encounter.difficulty_name.clone()
        .or_else(|| {
            if area.difficulty_name.is_empty() {
                None
            } else {
                Some(area.difficulty_name.clone())
            }
        });

    // Collect NPC names with counts (show count only if > 1)
    // Filter out companions - they're friendly NPCs, not enemies
    let mut npc_counts: HashMap<String, u32> = HashMap::new();
    for npc in encounter.npcs.values() {
        if npc.entity_type != EntityType::Companion {
            *npc_counts.entry(resolve(npc.name).to_string()).or_insert(0) += 1;
        }
    }
    let mut npc_names: Vec<String> = npc_counts
        .into_iter()
        .map(|(name, count)| {
            if count > 1 {
                format!("{} ({})", name, count)
            } else {
                name
            }
        })
        .collect();
    npc_names.sort();

    Some(EncounterSummary {
        encounter_id: encounter.id,
        display_name,
        encounter_type,
        start_time: encounter
            .enter_combat_time
            .map(|t| t.format("%Y-%m-%dT%H:%M:%S").to_string()),
        end_time: encounter
            .effective_end_time()
            .map(|t| t.format("%Y-%m-%dT%H:%M:%S").to_string()),
        duration_seconds: encounter.duration_seconds().unwrap_or(0),
        success: determine_success(encounter),
        area_name: encounter_area_name,
        difficulty,
        boss_name,
        player_metrics,
        is_phase_start,
        npc_names,
        // Line number tracking for per-encounter Parsely uploads
        // Use encounter's area_entered_line (set when combat started) instead of cache's current area
        // This ensures we get the correct AreaEntered line even if player exits to a different area
        area_entered_line: encounter.area_entered_line.or(area.entered_at_line),
        event_start_line: encounter.first_event_line,
        event_end_line: encounter.last_event_line,
        // Parsely link (set after successful upload)
        parsely_link: None,
    })
}
