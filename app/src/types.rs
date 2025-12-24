//! Frontend type definitions
//!
//! Contains types used by the Dioxus frontend, including re-exports from
//! baras-types and frontend-specific types that mirror backend structures.

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Re-exports from baras-types (shared with backend)
// ─────────────────────────────────────────────────────────────────────────────

pub use baras_types::{
    AppConfig, BossHealthConfig, Color, OverlayAppearanceConfig,
    OverlaySettings, PersonalOverlayConfig, PersonalStat, RaidOverlaySettings,
    TimerOverlayConfig, MAX_PROFILES,
};

// ─────────────────────────────────────────────────────────────────────────────
// Frontend-Only Types (mirror backend structures)
// ─────────────────────────────────────────────────────────────────────────────

/// Session information from the backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub player_name: Option<String>,
    pub player_class: Option<String>,
    pub player_discipline: Option<String>,
    pub area_name: Option<String>,
    pub in_combat: bool,
    pub encounter_count: usize,
    pub session_start: Option<String>,
}

/// Overlay status response from backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayStatus {
    pub running: Vec<String>,
    pub enabled: Vec<String>,
    pub personal_running: bool,
    pub personal_enabled: bool,
    pub raid_running: bool,
    pub raid_enabled: bool,
    pub boss_health_running: bool,
    pub boss_health_enabled: bool,
    pub timers_running: bool,
    pub timers_enabled: bool,
    pub effects_running: bool,
    pub effects_enabled: bool,
    pub overlays_visible: bool,
    pub move_mode: bool,
    pub rearrange_mode: bool,
}

/// Log file metadata for file browser
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogFileInfo {
    pub path: String,
    pub display_name: String,
    pub character_name: Option<String>,
    pub date: String,
    pub is_empty: bool,
    pub file_size: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Metric Types
// ─────────────────────────────────────────────────────────────────────────────

/// Available metric overlay types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetricType {
    Dps,
    EDps,
    BossDps,
    Hps,
    EHps,
    Abs,
    Dtps,
    Tps,
}

impl MetricType {
    /// Human-readable label for display
    pub fn label(&self) -> &'static str {
        match self {
            MetricType::Dps => "Damage",
            MetricType::EDps => "Effective Damage",
            MetricType::BossDps => "Boss Damage",
            MetricType::Hps => "Healing",
            MetricType::EHps => "Effective Healing",
            MetricType::Tps => "Threat",
            MetricType::Dtps => "Damage Taken",
            MetricType::Abs => "Shielding Given",
        }
    }

    /// Config key used for persistence
    pub fn config_key(&self) -> &'static str {
        match self {
            MetricType::Dps => "dps",
            MetricType::EDps => "edps",
            MetricType::BossDps => "bossdps",
            MetricType::Hps => "hps",
            MetricType::EHps => "ehps",
            MetricType::Tps => "tps",
            MetricType::Dtps => "dtps",
            MetricType::Abs => "abs",
        }
    }

    /// All metric overlay types (for iteration)
    pub fn all() -> &'static [MetricType] {
        &[
            MetricType::Dps,
            MetricType::EDps,
            MetricType::BossDps,
            MetricType::Hps,
            MetricType::EHps,
            MetricType::Abs,
            MetricType::Dtps,
            MetricType::Tps,
        ]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Overlay Type Enum
// ─────────────────────────────────────────────────────────────────────────────

/// Unified overlay kind - matches backend OverlayType
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum OverlayType {
    Metric(MetricType),
    Personal,
    Raid,
    BossHealth,
    Timers,
    Effects,
}

// ─────────────────────────────────────────────────────────────────────────────
// Timer Editor Types
// ─────────────────────────────────────────────────────────────────────────────

/// Flattened timer item for the timer editor list view
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimerListItem {
    // Identity
    pub timer_id: String,
    pub boss_id: String,
    pub boss_name: String,
    pub area_name: String,
    pub category: String,
    pub file_path: String,

    // Timer data
    pub name: String,
    pub enabled: bool,
    pub duration_secs: f32,
    pub color: [u8; 4],
    pub phases: Vec<String>,
    pub difficulties: Vec<String>,

    // Trigger info
    pub trigger: TimerTrigger,

    // Optional fields
    pub can_be_refreshed: bool,
    pub repeats: u8,
    pub chains_to: Option<String>,
    pub alert_at_secs: Option<f32>,
    pub show_on_raid_frames: bool,
}

/// Timer trigger types (mirrors backend BossTimerTrigger)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TimerTrigger {
    CombatStart,
    AbilityCast {
        #[serde(default)]
        ability_ids: Vec<u64>,
    },
    EffectApplied {
        #[serde(default)]
        effect_ids: Vec<u64>,
    },
    EffectRemoved {
        #[serde(default)]
        effect_ids: Vec<u64>,
    },
    TimerExpires {
        timer_id: String,
    },
    PhaseEntered {
        phase_id: String,
    },
    BossHpBelow {
        hp_percent: f32,
        #[serde(default)]
        npc_id: Option<i64>,
        #[serde(default)]
        boss_name: Option<String>,
    },
    AllOf {
        conditions: Vec<TimerTrigger>,
    },
    AnyOf {
        conditions: Vec<TimerTrigger>,
    },
}

impl TimerTrigger {
    /// Human-readable label for the trigger type
    pub fn label(&self) -> &'static str {
        match self {
            TimerTrigger::CombatStart => "Combat Start",
            TimerTrigger::AbilityCast { .. } => "Ability Cast",
            TimerTrigger::EffectApplied { .. } => "Effect Applied",
            TimerTrigger::EffectRemoved { .. } => "Effect Removed",
            TimerTrigger::TimerExpires { .. } => "Timer Expires",
            TimerTrigger::PhaseEntered { .. } => "Phase Entered",
            TimerTrigger::BossHpBelow { .. } => "Boss HP Below",
            TimerTrigger::AllOf { .. } => "All Of (AND)",
            TimerTrigger::AnyOf { .. } => "Any Of (OR)",
        }
    }

    /// Machine-readable type name for the trigger (matches serde tag)
    pub fn type_name(&self) -> &'static str {
        match self {
            TimerTrigger::CombatStart => "combat_start",
            TimerTrigger::AbilityCast { .. } => "ability_cast",
            TimerTrigger::EffectApplied { .. } => "effect_applied",
            TimerTrigger::EffectRemoved { .. } => "effect_removed",
            TimerTrigger::TimerExpires { .. } => "timer_expires",
            TimerTrigger::PhaseEntered { .. } => "phase_entered",
            TimerTrigger::BossHpBelow { .. } => "boss_hp_below",
            TimerTrigger::AllOf { .. } => "all_of",
            TimerTrigger::AnyOf { .. } => "any_of",
        }
    }
}

/// Minimal boss info for the "New Timer" dropdown
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BossListItem {
    pub id: String,
    pub name: String,
    pub area_name: String,
    pub category: String,
    pub file_path: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Effect Editor Types
// ─────────────────────────────────────────────────────────────────────────────

/// Effect category for display grouping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectCategory {
    #[default]
    Hot,
    Shield,
    Buff,
    Debuff,
    Cleansable,
    Proc,
    Mechanic,
}

impl EffectCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Hot => "HoT",
            Self::Shield => "Shield",
            Self::Buff => "Buff",
            Self::Debuff => "Debuff",
            Self::Cleansable => "Cleansable",
            Self::Proc => "Proc",
            Self::Mechanic => "Mechanic",
        }
    }

    pub fn all() -> &'static [EffectCategory] {
        &[
            Self::Hot,
            Self::Shield,
            Self::Buff,
            Self::Debuff,
            Self::Cleansable,
            Self::Proc,
            Self::Mechanic,
        ]
    }
}

/// Entity filter for source/target matching
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityFilter {
    #[default]
    LocalPlayer,
    LocalCompanion,
    LocalPlayerOrCompanion,
    OtherPlayers,
    OtherCompanions,
    AnyPlayer,
    AnyCompanion,
    AnyPlayerOrCompanion,
    GroupMembers,
    GroupMembersExceptLocal,
    Boss,
    NpcExceptBoss,
    AnyNpc,
    Specific(String),
    Any,
}

impl EntityFilter {
    pub fn label(&self) -> &'static str {
        match self {
            Self::LocalPlayer => "Local Player",
            Self::LocalCompanion => "Local Companion",
            Self::LocalPlayerOrCompanion => "Local Player or Companion",
            Self::OtherPlayers => "Other Players",
            Self::OtherCompanions => "Other Companions",
            Self::AnyPlayer => "Any Player",
            Self::AnyCompanion => "Any Companion",
            Self::AnyPlayerOrCompanion => "Any Player or Companion",
            Self::GroupMembers => "Group Members",
            Self::GroupMembersExceptLocal => "Group (Except Local)",
            Self::Boss => "Boss",
            Self::NpcExceptBoss => "NPC (Non-Boss)",
            Self::AnyNpc => "Any NPC",
            Self::Specific(_) => "Specific",
            Self::Any => "Any",
        }
    }

    /// Common filters for source field
    pub fn source_options() -> &'static [EntityFilter] {
        &[
            Self::LocalPlayer,
            Self::OtherPlayers,
            Self::AnyPlayer,
            Self::Boss,
            Self::AnyNpc,
            Self::Any,
        ]
    }

    /// Common filters for target field
    pub fn target_options() -> &'static [EntityFilter] {
        &[
            Self::LocalPlayer,
            Self::GroupMembers,
            Self::GroupMembersExceptLocal,
            Self::AnyPlayer,
            Self::Boss,
            Self::AnyNpc,
            Self::Any,
        ]
    }
}

/// Effect item for the effect editor list view (matches backend EffectListItem)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectListItem {
    // Identity
    pub id: String,
    pub name: String,
    pub file_path: String,

    // Core
    pub enabled: bool,
    pub category: EffectCategory,

    // Matching
    pub effect_ids: Vec<u64>,
    pub refresh_abilities: Vec<u64>,

    // Filtering
    pub source: EntityFilter,
    pub target: EntityFilter,

    // Duration
    pub duration_secs: Option<f32>,
    pub can_be_refreshed: bool,
    pub max_stacks: u8,

    // Display
    pub color: Option<[u8; 4]>,
    pub show_on_raid_frames: bool,
    pub show_on_effects_overlay: bool,

    // Behavior (advanced)
    pub persist_past_death: bool,
    pub track_outside_combat: bool,

    // Timer integration (advanced)
    pub on_apply_trigger_timer: Option<String>,
    pub on_expire_trigger_timer: Option<String>,

    // Context (advanced)
    pub encounters: Vec<String>,

    // Alerts (advanced)
    pub alert_near_expiration: bool,
    pub alert_threshold_secs: f32,
}
