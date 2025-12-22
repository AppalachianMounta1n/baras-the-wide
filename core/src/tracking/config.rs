//! Configuration loading for effect and timer definitions
//!
//! Definitions are loaded from TOML files in two locations:
//! - **Builtin**: Shipped with the application (read-only)
//! - **Custom**: User-created definitions (editable)
//!
//! Builtin definitions use reserved ID prefixes to avoid collisions.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::definitions::{DefinitionConfig, EffectDefinition, TimerDefinition};

/// Combined set of effect and timer definitions
#[derive(Debug, Clone, Default)]
pub struct DefinitionSet {
    /// All effect definitions, keyed by ID
    pub effects: HashMap<String, EffectDefinition>,

    /// All timer definitions, keyed by ID
    pub timers: HashMap<String, TimerDefinition>,
}

impl DefinitionSet {
    /// Create an empty definition set
    pub fn new() -> Self {
        Self::default()
    }

    /// Add definitions from a config, returns IDs of any duplicates
    pub fn add_config(&mut self, config: DefinitionConfig) -> Vec<String> {
        let mut duplicates = Vec::new();

        for effect in config.effects {
            if self.effects.contains_key(&effect.id) {
                duplicates.push(effect.id.clone());
            }
            self.effects.insert(effect.id.clone(), effect);
        }

        for timer in config.timers {
            if self.timers.contains_key(&timer.id) {
                duplicates.push(timer.id.clone());
            }
            self.timers.insert(timer.id.clone(), timer);
        }

        duplicates
    }

    /// Get an effect definition by ID
    pub fn get_effect(&self, id: &str) -> Option<&EffectDefinition> {
        self.effects.get(id)
    }

    /// Get a timer definition by ID
    pub fn get_timer(&self, id: &str) -> Option<&TimerDefinition> {
        self.timers.get(id)
    }

    /// Find effect definitions that match a game effect ID
    pub fn find_effects_by_game_id(&self, effect_id: u64) -> Vec<&EffectDefinition> {
        self.effects
            .values()
            .filter(|def| def.enabled && def.matches_effect(effect_id))
            .collect()
    }

    /// Get all enabled effect definitions
    pub fn enabled_effects(&self) -> impl Iterator<Item = &EffectDefinition> {
        self.effects.values().filter(|def| def.enabled)
    }

    /// Get all enabled timer definitions
    pub fn enabled_timers(&self) -> impl Iterator<Item = &TimerDefinition> {
        self.timers.values().filter(|def| def.enabled)
    }
}

/// Load definitions from builtin and custom config directories
///
/// # Arguments
/// * `builtin_dir` - Directory containing builtin TOML files (shipped with app)
/// * `custom_dir` - Directory containing user TOML files (optional)
///
/// # Returns
/// A `DefinitionSet` with all loaded definitions merged together.
/// Builtin definitions are loaded first, then custom definitions.
/// Custom definitions with the same ID will override builtins.
pub fn load_definitions(
    builtin_dir: Option<&Path>,
    custom_dir: Option<&Path>,
) -> Result<DefinitionSet, ConfigError> {
    let mut set = DefinitionSet::new();

    // Load builtin definitions first
    if let Some(dir) = builtin_dir {
        if dir.exists() {
            load_directory(&mut set, dir, "builtin")?;
        }
    }

    // Load custom definitions (can override builtins)
    if let Some(dir) = custom_dir {
        if dir.exists() {
            load_directory(&mut set, dir, "custom")?;
        }
    }

    Ok(set)
}

/// Load all TOML files from a directory
fn load_directory(set: &mut DefinitionSet, dir: &Path, source: &str) -> Result<(), ConfigError> {
    let entries = fs::read_dir(dir).map_err(|e| ConfigError::IoError {
        path: dir.to_path_buf(),
        source: e,
    })?;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.extension().is_some_and(|ext| ext == "toml") {
            match load_file(&path) {
                Ok(config) => {
                    let duplicates = set.add_config(config);
                    if !duplicates.is_empty() {
                        // Log warning about duplicates but continue
                        eprintln!(
                            "[{}] Duplicate definition IDs in {:?}: {:?}",
                            source,
                            path.file_name(),
                            duplicates
                        );
                    }
                }
                Err(e) => {
                    // Log error but continue loading other files
                    eprintln!("[{}] Failed to load {:?}: {}", source, path.file_name(), e);
                }
            }
        }
    }

    Ok(())
}

/// Load a single TOML config file
pub fn load_file(path: &Path) -> Result<DefinitionConfig, ConfigError> {
    let contents = fs::read_to_string(path).map_err(|e| ConfigError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    toml::from_str(&contents).map_err(|e| ConfigError::ParseError {
        path: path.to_path_buf(),
        source: e,
    })
}

/// Save a config to a TOML file
pub fn save_file(path: &Path, config: &DefinitionConfig) -> Result<(), ConfigError> {
    let contents = toml::to_string_pretty(config).map_err(|e| ConfigError::SerializeError {
        path: path.to_path_buf(),
        source: e,
    })?;

    fs::write(path, contents).map_err(|e| ConfigError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    Ok(())
}

/// Get the default builtin definitions directory
pub fn default_builtin_dir() -> Option<PathBuf> {
    // Look relative to executable, or use compile-time path
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("definitions").join("builtin")))
}

/// Get the default custom definitions directory
pub fn default_custom_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("baras").join("definitions"))
}

/// Errors that can occur during config loading
#[derive(Debug)]
pub enum ConfigError {
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseError {
        path: PathBuf,
        source: toml::de::Error,
    },
    SerializeError {
        path: PathBuf,
        source: toml::ser::Error,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError { path, source } => {
                write!(f, "IO error reading {:?}: {}", path, source)
            }
            Self::ParseError { path, source } => {
                write!(f, "Parse error in {:?}: {}", path, source)
            }
            Self::SerializeError { path, source } => {
                write!(f, "Serialize error for {:?}: {}", path, source)
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IoError { source, .. } => Some(source),
            Self::ParseError { source, .. } => Some(source),
            Self::SerializeError { source, .. } => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_effect_toml() {
        let toml = r#"
[[effect]]
id = "kolto_probe"
name = "Kolto Probe"
effect_ids = [814832605462528]
refresh_abilities = [814832605462528, 1014376786034688]
duration_secs = 20.0
category = "hot"
max_stacks = 2
"#;

        let config: DefinitionConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.effects.len(), 1);
        assert_eq!(config.effects[0].id, "kolto_probe");
        assert_eq!(config.effects[0].effect_ids, vec![814832605462528]);
    }

    #[test]
    fn test_parse_timer_toml() {
        let toml = r#"
[[timer]]
id = "boss_enrage"
name = "Enrage"
duration_secs = 300.0
color = [255, 0, 0, 255]

[timer.trigger]
type = "combat_start"
"#;

        let config: DefinitionConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.timers.len(), 1);
        assert_eq!(config.timers[0].id, "boss_enrage");
        assert_eq!(config.timers[0].duration_secs, 300.0);
    }
}
