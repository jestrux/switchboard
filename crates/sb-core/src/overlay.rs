//! Platform and machine overlays: `.switchboard/project.<os>.yaml` and
//! `.switchboard/project.local.yaml` in a project's home folder.
//!
//! An overlay lists only what differs on this platform or machine, keyed by group and
//! command id. Overlays are applied when a config is composed, never saved into it, so
//! write-back to the shared `project.yaml` cannot pick them up.
//!
//! ```yaml
//! # .switchboard/project.linux.yaml
//! commands:
//!   claude: { cwd: ~/Work/otesha }
//! groups:
//!   console:
//!     commands:
//!       dev: { run: npm run dev -- --port 5301 }
//!   mobile:
//!     commands:
//!       ios: { hidden: true }
//! ```
//!
//! Order: `project.yaml`, then `project.<os>.yaml`, then `project.local.yaml`; later wins.
//! Relative paths in an overlay resolve from the home folder, like `project.yaml`'s.

use crate::config::{collapse_tilde, CommandConfig, ConfigError, GroupConfig, ProjectConfig};
use crate::fragment::{resolve_folder, DOT_DIR};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const LOCAL_FILE: &str = "project.local.yaml";

/// `macos`, `linux`, … as Rust names them.
pub fn platform() -> &'static str {
    std::env::consts::OS
}

/// `project.<os>.yaml` for this platform.
pub fn platform_file() -> String {
    format!("project.{}.yaml", platform())
}

/// The overlay files for `home`, in the order they apply (whether or not they exist).
pub fn paths_in(home: &Path) -> Vec<PathBuf> {
    let dir = home.join(DOT_DIR);
    vec![dir.join(platform_file()), dir.join(LOCAL_FILE)]
}

#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Overlay {
    /// Ungrouped commands by id.
    #[serde(default)]
    pub commands: BTreeMap<String, CommandPatch>,
    #[serde(default)]
    pub groups: BTreeMap<String, GroupPatch>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct GroupPatch {
    pub name: Option<String>,
    pub cwd: Option<String>,
    pub from: Option<String>,
    pub icon: Option<String>,
    /// Drop the whole group on this platform.
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub commands: BTreeMap<String, CommandPatch>,
}

/// Fields set here replace the command's; `env` entries are merged in.
/// A patch with `run` for an id the project lacks adds that command.
#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct CommandPatch {
    pub name: Option<String>,
    pub run: Option<String>,
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub autostart: Option<bool>,
    pub restart_on_crash: Option<bool>,
    #[serde(default)]
    pub hidden: bool,
}

impl Overlay {
    pub fn from_yaml(text: &str) -> Result<Overlay, ConfigError> {
        serde_yaml::from_str(text).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    pub fn load(path: &Path) -> Result<Overlay, ConfigError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Io(format!("{}: {e}", collapse_tilde(path))))?;
        Self::from_yaml(&text).map_err(|e| match e {
            ConfigError::Parse(m) => ConfigError::Parse(format!("{}: {m}", collapse_tilde(path))),
            other => other,
        })
    }

    /// Group-level changes, applied before `from:` groups are expanded so an overlay can
    /// move a group's folder. Paths are written out resolved against `home`.
    pub fn apply_groups(&self, cfg: &mut ProjectConfig, home: &Path) {
        let path = |raw: &str| collapse_tilde(&resolve_folder(raw, home));
        cfg.groups.retain(|g| !self.groups.get(&g.id).map(|p| p.hidden).unwrap_or(false));
        for (id, patch) in &self.groups {
            if patch.hidden {
                continue;
            }
            let group = match cfg.groups.iter_mut().find(|g| &g.id == id) {
                Some(g) => g,
                None => {
                    cfg.groups.push(GroupConfig {
                        id: id.clone(),
                        name: None,
                        cwd: None,
                        from: None,
                        overrides: BTreeMap::new(),
                        icon: None,
                        commands: Vec::new(),
                        extra: Default::default(),
                    });
                    cfg.groups.last_mut().unwrap()
                }
            };
            if patch.name.is_some() { group.name = patch.name.clone(); }
            if patch.icon.is_some() { group.icon = patch.icon.clone(); }
            if let Some(c) = &patch.cwd { group.cwd = Some(path(c)); }
            if let Some(f) = &patch.from { group.from = Some(path(f)); }
        }
    }

    /// Command-level changes, applied after `from:` groups are expanded so fragment
    /// commands can be patched too.
    pub fn apply_commands(&self, cfg: &mut ProjectConfig, home: &Path) {
        apply_to(&mut cfg.commands, &self.commands, home);
        for group in &mut cfg.groups {
            if let Some(patch) = self.groups.get(&group.id) {
                apply_to(&mut group.commands, &patch.commands, home);
            }
        }
    }
}

fn apply_to(cmds: &mut Vec<CommandConfig>, patches: &BTreeMap<String, CommandPatch>, home: &Path) {
    let path = |raw: &str| collapse_tilde(&resolve_folder(raw, home));
    cmds.retain(|c| !patches.get(&c.id).map(|p| p.hidden).unwrap_or(false));
    for (id, p) in patches {
        if p.hidden {
            continue;
        }
        match cmds.iter_mut().find(|c| &c.id == id) {
            Some(cmd) => {
                if p.name.is_some() { cmd.name = p.name.clone(); }
                if let Some(r) = &p.run { cmd.run = r.clone(); }
                if let Some(c) = &p.cwd { cmd.cwd = Some(path(c)); }
                if p.autostart.is_some() { cmd.autostart = p.autostart; }
                if p.restart_on_crash.is_some() { cmd.restart_on_crash = p.restart_on_crash; }
                for (k, v) in &p.env { cmd.env.insert(k.clone(), v.clone()); }
            }
            // Only a patch that says what to run can add a command.
            None => {
                let Some(run) = &p.run else { continue };
                cmds.push(CommandConfig {
                    id: id.clone(),
                    name: p.name.clone(),
                    run: run.clone(),
                    cwd: p.cwd.as_deref().map(path),
                    env: p.env.clone(),
                    autostart: p.autostart,
                    restart_on_crash: p.restart_on_crash,
                    terminal: false,
                    extra: Default::default(),
                });
            }
        }
    }
}
