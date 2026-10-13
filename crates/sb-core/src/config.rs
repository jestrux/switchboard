//! `config.yaml` schema (v1), validation and command resolution.

use serde::{Deserialize, Serialize};
use serde_yaml::Mapping;
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

/// Top-level project config. Unknown keys are kept in `extra` so hand edits survive a save.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectConfig {
    pub version: u32,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Ungrouped commands, shown above the groups.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<CommandConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<GroupConfig>,
    #[serde(flatten)]
    pub extra: Mapping,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GroupConfig {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<CommandConfig>,
    #[serde(flatten)]
    pub extra: Mapping,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandConfig {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub run: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// `None` means "not set", which defaults to `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autostart: Option<bool>,
    /// `None` means "not set", which defaults to `false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart_on_crash: Option<bool>,
    #[serde(flatten)]
    pub extra: Mapping,
}

impl CommandConfig {
    pub fn autostart(&self) -> bool {
        self.autostart.unwrap_or(true)
    }
    pub fn restart_on_crash(&self) -> bool {
        self.restart_on_crash.unwrap_or(false)
    }
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}

impl GroupConfig {
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}

/// The group a resolved command belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GroupRef {
    pub id: String,
    pub name: String,
}

/// A command with its group, folder and defaults resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedCommand {
    /// `group/command` or just `command` when ungrouped.
    pub address: String,
    pub id: String,
    /// The command's own display name, e.g. `make:migrate`.
    pub name: String,
    /// Prefixed display name, e.g. `Backend / make:migrate`.
    pub display_name: String,
    pub group: Option<GroupRef>,
    pub run: String,
    /// The `cwd` as written (own, else group's), before `~` expansion.
    pub cwd_raw: Option<String>,
    /// Expanded working folder. `None` when neither the command nor its group set one.
    pub cwd: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
    pub autostart: bool,
    pub restart_on_crash: bool,
}

impl ResolvedCommand {
    fn new(cmd: &CommandConfig, group: Option<&GroupConfig>) -> Self {
        let cwd_raw = cmd
            .cwd
            .clone()
            .or_else(|| group.and_then(|g| g.cwd.clone()));
        let name = cmd.display_name().to_string();
        let (address, display_name, group_ref) = match group {
            Some(g) => (
                format!("{}/{}", g.id, cmd.id),
                format!("{} / {}", g.display_name(), name),
                Some(GroupRef {
                    id: g.id.clone(),
                    name: g.display_name().to_string(),
                }),
            ),
            None => (cmd.id.clone(), name.clone(), None),
        };
        ResolvedCommand {
            address,
            id: cmd.id.clone(),
            name,
            display_name,
            group: group_ref,
            run: cmd.run.clone(),
            cwd: cwd_raw.as_deref().map(expand_tilde),
            cwd_raw,
            env: cmd.env.clone(),
            autostart: cmd.autostart(),
            restart_on_crash: cmd.restart_on_crash(),
        }
    }
}

impl ProjectConfig {
    /// Parse YAML text. Does not validate.
    pub fn from_yaml(text: &str) -> Result<Self, ConfigError> {
        serde_yaml::from_str(text).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Parse and validate a `config.yaml` file.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Io(format!("{}: {e}", path.display())))?;
        let cfg = Self::from_yaml(&text)?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn to_yaml(&self) -> Result<String, ConfigError> {
        serde_yaml::to_string(self).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Check the schema rules. Returns every problem found, not just the first.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let mut errors = Vec::new();
        if self.version != 1 {
            errors.push(format!("version must be 1 (found {})", self.version));
        }
        if self.name.trim().is_empty() {
            errors.push("name must not be empty".to_string());
        }
        if let Some(color) = &self.color {
            if !is_hex_color(color) {
                errors.push(format!("color must look like \"#RRGGBB\" (found {color:?})"));
            }
        }
        let mut seen_groups = HashSet::new();
        validate_commands(&self.commands, None, &mut errors);
        for group in &self.groups {
            let gid = group.id.trim();
            if gid.is_empty() {
                errors.push("group id must not be empty".to_string());
                continue;
            }
            if gid.contains('/') {
                errors.push(format!("group id {gid:?} must not contain '/'"));
            }
            if !seen_groups.insert(gid.to_string()) {
                errors.push(format!("duplicate group id {gid:?}"));
            }
            validate_commands(&group.commands, Some(gid), &mut errors);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::Invalid(errors))
        }
    }

    /// Every command in sidebar order: ungrouped first, then each group in file order.
    pub fn resolve_commands(&self) -> Vec<ResolvedCommand> {
        let mut out = Vec::new();
        for cmd in &self.commands {
            out.push(ResolvedCommand::new(cmd, None));
        }
        for group in &self.groups {
            for cmd in &group.commands {
                out.push(ResolvedCommand::new(cmd, Some(group)));
            }
        }
        out
    }

    /// Look a command up by address (`group/command` or `command`).
    pub fn resolve(&self, address: &str) -> Option<ResolvedCommand> {
        self.resolve_commands().into_iter().find(|c| c.address == address)
    }

    /// Commands in one group, in file order.
    pub fn resolve_group(&self, group_id: &str) -> Vec<ResolvedCommand> {
        self.resolve_commands()
            .into_iter()
            .filter(|c| c.group.as_ref().map(|g| g.id.as_str()) == Some(group_id))
            .collect()
    }

    pub fn group(&self, group_id: &str) -> Option<&GroupConfig> {
        self.groups.iter().find(|g| g.id == group_id)
    }
}

fn validate_commands(cmds: &[CommandConfig], group: Option<&str>, errors: &mut Vec<String>) {
    let scope = match group {
        Some(g) => format!("group {g:?}"),
        None => "ungrouped commands".to_string(),
    };
    let mut seen = HashSet::new();
    for cmd in cmds {
        let id = cmd.id.trim();
        if id.is_empty() {
            errors.push(format!("command id must not be empty (in {scope})"));
            continue;
        }
        if id.contains('/') {
            errors.push(format!("command id {id:?} must not contain '/' (in {scope})"));
        }
        if !seen.insert(id.to_string()) {
            errors.push(format!("duplicate command id {id:?} in {scope}"));
        }
        if cmd.run.trim().is_empty() {
            errors.push(format!("command {id:?} has an empty run"));
        }
    }
}

fn is_hex_color(s: &str) -> bool {
    let Some(hex) = s.strip_prefix('#') else { return false };
    (hex.len() == 6 || hex.len() == 8) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

/// Expand a leading `~` or `~/` to the home folder.
pub fn expand_tilde(path: &str) -> PathBuf {
    if path == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
    }
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// Collapse a path under the home folder to `~/…` for storage.
pub fn collapse_tilde(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rest) = path.strip_prefix(&home) {
            if rest.as_os_str().is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    Io(String),
    Parse(String),
    Invalid(Vec<String>),
}

impl ConfigError {
    /// Every message, one per line.
    pub fn messages(&self) -> Vec<String> {
        match self {
            ConfigError::Io(m) | ConfigError::Parse(m) => vec![m.clone()],
            ConfigError::Invalid(v) => v.clone(),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(m) => write!(f, "{m}"),
            ConfigError::Parse(m) => write!(f, "YAML error: {m}"),
            ConfigError::Invalid(v) => write!(f, "{}", v.join("\n")),
        }
    }
}

impl std::error::Error for ConfigError {}
