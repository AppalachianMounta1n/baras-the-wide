//! Effect and Timer definition types
//!
//! Definitions are templates loaded from TOML config files that describe
//! what effects/timers to track and how to display them.

use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════
// Entity Filter (shared by source and target)
// ═══════════════════════════════════════════════════════════════════════════

/// Filter for matching entities (used for both source and target)
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityFilter {
    /// The local player only
    #[default]
    LocalPlayer,
    /// Local player's companion
    LocalCompanion,
    /// Local player OR their companion
    LocalPlayerOrCompanion,
    /// Other players (not local)
    OtherPlayers,
    /// Other players' companions
    OtherCompanions,
    /// Any player (including local)
    AnyPlayer,
    /// Any companion (any player's)
    AnyCompanion,
    /// Any player or companion
    AnyPlayerOrCompanion,
    /// Group members (players in the local player's group)
    GroupMembers,
    /// Group members except local player
    GroupMembersExceptLocal,
    /// Boss NPCs specifically
    Boss,
    /// Non-boss NPCs (trash mobs)
    NpcExceptBoss,
    /// Any NPC (boss or trash)
    AnyNpc,
    /// Specific entity by name
    Specific(String),
    /// Any entity whatsoever
    Any,
}

// ═══════════════════════════════════════════════════════════════════════════
// Effect Definitions
// ═══════════════════════════════════════════════════════════════════════════

/// How an effect should be categorized and displayed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectCategory {
    /// Heal over Time (default green)
    #[default]
    Hot,
    /// Absorb shield/barrier (yellow/gold)
    Shield,
    /// Beneficial buff (blue)
    Buff,
    /// Harmful debuff (red)
    Debuff,
    /// Dispellable/cleansable effect (purple)
    Cleansable,
    /// Temporary proc (cyan)
    Proc,
    /// Boss mechanic on player (orange)
    Mechanic,
}

impl EffectCategory {
    /// Default RGBA color for this category
    pub fn default_color(&self) -> [u8; 4] {
        match self {
            Self::Hot => [80, 200, 80, 255],         // Green
            Self::Shield => [220, 180, 50, 255],     // Yellow/Gold
            Self::Buff => [80, 140, 220, 255],       // Blue
            Self::Debuff => [200, 60, 60, 255],      // Red
            Self::Cleansable => [180, 80, 200, 255], // Purple
            Self::Proc => [80, 200, 220, 255],       // Cyan
            Self::Mechanic => [255, 140, 60, 255],   // Orange
        }
    }
}

/// Definition of an effect to track (loaded from config)
///
/// This is the "template" that describes what game effect to watch for
/// and how to display it. Multiple `ActiveEffect` instances may be
/// created from a single definition (one per affected player).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectDefinition {
    /// Unique identifier for this definition (e.g., "kolto_probe")
    pub id: String,

    /// Display name shown in overlays
    pub name: String,

    /// Whether this definition is currently enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    // ─── Matching ───────────────────────────────────────────────────────────
    /// Game effect IDs that match this definition
    #[serde(default)]
    pub effect_ids: Vec<u64>,

    /// Ability IDs that can apply or refresh this effect
    #[serde(default)]
    pub refresh_abilities: Vec<u64>,

    // ─── Filtering ──────────────────────────────────────────────────────────
    /// Who must apply the effect for it to be tracked
    #[serde(default)]
    pub source: EntityFilter,

    /// Who must receive the effect for it to be tracked
    #[serde(default)]
    pub target: EntityFilter,

    // ─── Duration ───────────────────────────────────────────────────────────
    /// Expected duration in seconds (None = indefinite/unknown)
    pub duration_secs: Option<f32>,

    /// Can this effect be refreshed by reapplication?
    #[serde(default = "default_true")]
    pub can_be_refreshed: bool,

    // ─── Display ────────────────────────────────────────────────────────────
    /// Effect category (determines default color)
    #[serde(default)]
    pub category: EffectCategory,

    /// Override color as RGBA (None = use category default)
    pub color: Option<[u8; 4]>,

    /// Maximum stacks to display (0 = don't show stacks)
    #[serde(default)]
    pub max_stacks: u8,

    // ─── Behavior ───────────────────────────────────────────────────────────
    /// Should this effect persist after target dies?
    #[serde(default)]
    pub persist_past_death: bool,

    /// Track this effect outside of combat?
    #[serde(default = "default_true")]
    pub track_outside_combat: bool,

    // ─── Timer Integration ──────────────────────────────────────────────────
    /// Timer ID to start when this effect is applied
    pub on_apply_trigger_timer: Option<String>,

    /// Timer ID to start when this effect expires/is removed
    pub on_expire_trigger_timer: Option<String>,

    // ─── Context ────────────────────────────────────────────────────────────
    /// Only track in specific encounters (empty = all encounters)
    #[serde(default)]
    pub encounters: Vec<String>,

    // ─── Alerts ─────────────────────────────────────────────────────────────
    /// Show visual warning when effect is about to expire
    #[serde(default)]
    pub alert_near_expiration: bool,

    /// Seconds before expiration to show warning
    #[serde(default = "default_alert_threshold")]
    pub alert_threshold_secs: f32,
}

impl EffectDefinition {
    /// Get the effective color (override or category default)
    pub fn effective_color(&self) -> [u8; 4] {
        self.color.unwrap_or_else(|| self.category.default_color())
    }

    /// Check if an effect ID matches this definition
    pub fn matches_effect(&self, effect_id: u64) -> bool {
        self.effect_ids.contains(&effect_id)
    }

    /// Check if an ability can refresh this effect
    pub fn can_refresh_with(&self, ability_id: u64) -> bool {
        self.refresh_abilities.contains(&ability_id)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Timer Definitions
// ═══════════════════════════════════════════════════════════════════════════

/// What triggers a timer to start
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum TimerTrigger {
    /// Combat starts
    CombatStart,

    /// Specific ability is cast
    AbilityCast {
        /// Ability IDs that trigger this timer
        ability_ids: Vec<u64>,
    },

    /// Effect is applied to someone
    EffectApplied {
        /// Effect IDs that trigger this timer
        effect_ids: Vec<u64>,
    },

    /// Effect is removed from someone
    EffectRemoved {
        /// Effect IDs that trigger this timer
        effect_ids: Vec<u64>,
    },

    /// Another timer expires (chaining)
    TimerExpires {
        /// ID of the timer that triggers this one
        timer_id: String,
    },

    /// Boss HP reaches a threshold
    BossHpThreshold {
        /// HP percentage (0.0 - 100.0)
        hp_percent: f32,
    },

    /// Manually triggered (for testing/debug)
    Manual,
}

/// Definition of a timer (loaded from config)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerDefinition {
    /// Unique identifier for this timer
    pub id: String,

    /// Display name shown in overlays
    pub name: String,

    /// Whether this timer is currently enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    // ─── Trigger ────────────────────────────────────────────────────────────
    /// What causes this timer to start
    pub trigger: TimerTrigger,

    /// Source filter for trigger events
    #[serde(default)]
    pub source: EntityFilter,

    /// Target filter for trigger events
    #[serde(default)]
    pub target: EntityFilter,

    // ─── Duration ───────────────────────────────────────────────────────────
    /// Timer duration in seconds
    pub duration_secs: f32,

    /// If true, resets duration when triggered again
    #[serde(default)]
    pub can_be_refreshed: bool,

    /// Number of times this repeats after initial trigger (0 = no repeat)
    #[serde(default)]
    pub repeats: u8,

    // ─── Display ────────────────────────────────────────────────────────────
    /// Display color as RGBA
    #[serde(default = "default_timer_color")]
    pub color: [u8; 4],

    /// Show on raid frames instead of timer bar overlay?
    #[serde(default)]
    pub show_on_raid_frames: bool,

    // ─── Alerts ─────────────────────────────────────────────────────────────
    /// Alert when this many seconds remain (None = no alert)
    pub alert_at_secs: Option<f32>,

    /// Custom alert text (None = use timer name)
    pub alert_text: Option<String>,

    /// Audio file to play on alert
    pub audio_file: Option<String>,

    // ─── Chaining ───────────────────────────────────────────────────────────
    /// Timer ID to trigger when this one expires
    pub triggers_timer: Option<String>,

    // ─── Context ────────────────────────────────────────────────────────────
    /// Only active in specific encounters (empty = all)
    #[serde(default)]
    pub encounters: Vec<String>,

    /// Specific boss name (if applicable)
    pub boss: Option<String>,

    /// Active difficulties: "story", "veteran", "master"
    #[serde(default)]
    pub difficulties: Vec<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Serde Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn default_true() -> bool {
    true
}

fn default_alert_threshold() -> f32 {
    3.0
}

fn default_timer_color() -> [u8; 4] {
    [200, 200, 200, 255] // Light grey
}

// ═══════════════════════════════════════════════════════════════════════════
// Config File Structure
// ═══════════════════════════════════════════════════════════════════════════

/// Root structure for effect/timer config files
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DefinitionConfig {
    /// Effect definitions in this file
    #[serde(default, rename = "effect")]
    pub effects: Vec<EffectDefinition>,

    /// Timer definitions in this file
    #[serde(default, rename = "timer")]
    pub timers: Vec<TimerDefinition>,
}
