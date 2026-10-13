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
    /// Look for this project's own window: a built-in name (`graphite`, `patch-bay`,
    /// `paper`) or a custom theme with token overrides. The main board keeps the
    /// app-wide setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemeSpec>,
    /// `system`, `light` or `dark`, for this project's own window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<String>,
    /// The repo folder whose `.switchboard/project.yaml` mirrors this config (write-back).
    /// Relative `from:` paths resolve against its parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<String>,
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
    /// A repo folder whose `.switchboard/commands.yaml` supplies this group's commands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Per-command tweaks over the fragment's commands, by command id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<String, crate::fragment::CommandOverride>,
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
    /// An ad-hoc terminal (interactive login shell) rather than a configured command.
    /// Never read from or written to YAML.
    #[serde(skip)]
    pub terminal: bool,
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
    /// Interactive shell session (ad-hoc terminal) instead of `$SHELL -lc run`.
    pub terminal: bool,
}

impl ResolvedCommand {
    fn new(cmd: &CommandConfig, group: Option<&GroupConfig>) -> Self {
        let cwd_raw = cmd
            .cwd
            .clone()
            .or_else(|| group.and_then(|g| g.cwd.clone()));
        let name = cmd.display_name().to_string();
        let (address, display_name, group_ref) = match group {
            // Terminals carry their own `@terminal/<id>` address regardless of group.
            Some(_) if cmd.terminal => (
                cmd.id.clone(),
                format!("{} / {}", group.unwrap().display_name(), name),
                group.map(|g| GroupRef { id: g.id.clone(), name: g.display_name().to_string() }),
            ),
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
            terminal: cmd.terminal,
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
        if let Some(t) = &self.theme {
            validate_theme(t, &mut errors);
        }
        if let Some(a) = &self.appearance {
            if !APPEARANCES.contains(&a.as_str()) {
                errors.push(format!("appearance must be one of {} (found {a:?})", APPEARANCES.join(", ")));
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

/// Theme ids a project may pin for its own window.
pub const THEMES: [&str; 3] = ["graphite", "patch-bay", "paper"];

/// Palette tokens a custom theme may override (plus `ansi`, a list of 16 colours).
pub const THEME_TOKENS: [&str; 32] = [
    "window", "rail", "sidebar", "line", "pane", "header", "terminal", "terminalText",
    "terminalDim", "text", "text2", "text3", "text4", "hover", "selection", "selectionText",
    "accent", "onAccent", "accentSoft", "accentSoftText", "button", "buttonLine",
    "running", "runningSoft", "starting", "startingSoft", "crashed", "crashedSoft",
    "error", "errorSoft", "stopped", "stoppedSoft",
];

/// `theme: patch-bay` or a custom block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ThemeSpec {
    Name(String),
    Custom(CustomTheme),
}

impl ThemeSpec {
    /// The built-in theme this resolves to or inherits from.
    pub fn base(&self) -> &str {
        match self {
            ThemeSpec::Name(n) => n,
            ThemeSpec::Custom(c) => c.base.as_deref().unwrap_or("graphite"),
        }
    }
}

/// A custom theme: a base to inherit from plus per-scheme token overrides.
///
/// ```yaml
/// theme:
///   name: CityBuild
///   base: graphite
///   dark:
///     accent: "#F7AD3F"
///     ansi: ["#2B2830", "#C4393C", …]   # 16 colours
///   light:
///     accent: "#981C1E"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomTheme {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dark: Option<BTreeMap<String, serde_yaml::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<BTreeMap<String, serde_yaml::Value>>,
}

fn validate_theme(spec: &ThemeSpec, errors: &mut Vec<String>) {
    let base = spec.base();
    if !THEMES.contains(&base) {
        errors.push(format!("theme must be one of {} (found {base:?})", THEMES.join(", ")));
    }
    if let ThemeSpec::Custom(c) = spec {
        for (scheme, tokens) in [("dark", &c.dark), ("light", &c.light)] {
            let Some(tokens) = tokens else { continue };
            for (key, value) in tokens {
                if key == "ansi" {
                    match value.as_sequence() {
                        Some(list) if list.len() == 16 => {
                            for (i, v) in list.iter().enumerate() {
                                if !v.as_str().map(is_hex_color).unwrap_or(false) {
                                    errors.push(format!("theme.{scheme}.ansi[{i}] must be a \"#RRGGBB\" colour"));
                                }
                            }
                        }
                        _ => errors.push(format!("theme.{scheme}.ansi must be a list of 16 colours")),
                    }
                } else if !THEME_TOKENS.contains(&key.as_str()) {
                    errors.push(format!("theme.{scheme}.{key} is not a theme token"));
                } else if !value.as_str().map(is_hex_color).unwrap_or(false) {
                    errors.push(format!("theme.{scheme}.{key} must be a \"#RRGGBB\" colour"));
                }
            }
        }
    }
}
/// Appearance values a project may pin for its own window.
pub const APPEARANCES: [&str; 3] = ["system", "light", "dark"];

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
