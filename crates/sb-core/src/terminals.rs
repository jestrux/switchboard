//! Ad-hoc terminals: shells opened in a group's folder, kept per project in
//! `terminals.yaml` next to `config.yaml` (device state, not project config).

use crate::config::{CommandConfig, ConfigError, ProjectConfig};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const FILE_NAME: &str = "terminals.yaml";
/// Address prefix for terminals: `@terminal/<id>`.
pub const ADDRESS_PREFIX: &str = "@terminal/";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalEntry {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Folder the shell starts in; defaults to the group's folder at open time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

impl TerminalEntry {
    pub fn address(&self) -> String {
        format!("{ADDRESS_PREFIX}{}", self.id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TerminalsFile {
    #[serde(default)]
    pub terminals: Vec<TerminalEntry>,
}

pub fn load(project_dir: &Path) -> Vec<TerminalEntry> {
    let path = project_dir.join(FILE_NAME);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_yaml::from_str::<TerminalsFile>(&t).ok())
        .map(|f| f.terminals)
        .unwrap_or_default()
}

pub fn save(project_dir: &Path, entries: &[TerminalEntry]) -> Result<(), ConfigError> {
    let path = project_dir.join(FILE_NAME);
    if entries.is_empty() {
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }
    let text = serde_yaml::to_string(&TerminalsFile { terminals: entries.to_vec() })
        .map_err(|e| ConfigError::Parse(e.to_string()))?;
    std::fs::write(path, text).map_err(|e| ConfigError::Io(e.to_string()))
}

/// Terminal id from an address, if it is one.
pub fn id_from_address(address: &str) -> Option<&str> {
    address.strip_prefix(ADDRESS_PREFIX)
}

/// A short random id.
pub fn new_id() -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id() as u128;
    format!("{:06x}", (n ^ (pid << 20)) & 0xff_ffff)
}

/// Default name: "Terminal", then "Terminal 2", … within the group.
pub fn next_name(existing: &[TerminalEntry], group: Option<&str>) -> String {
    let n = existing.iter().filter(|t| t.group.as_deref() == group).count();
    if n == 0 { "Terminal".to_string() } else { format!("Terminal {}", n + 1) }
}

/// Add the terminals to a composed config as commands so the rest of the engine treats
/// them like any other session. They are marked `terminal` and never serialised back.
pub fn merge_into(config: &mut ProjectConfig, entries: &[TerminalEntry]) {
    for t in entries {
        let cmd = CommandConfig {
            id: t.address(),
            name: Some(t.name.clone()),
            run: crate::session::login_shell(),
            cwd: t.cwd.clone(),
            env: Default::default(),
            autostart: Some(false),
            restart_on_crash: Some(false),
            terminal: true,
            extra: Default::default(),
        };
        match t.group.as_deref().and_then(|g| config.groups.iter_mut().find(|x| x.id == g)) {
            Some(group) => group.commands.push(cmd),
            None => config.commands.push(cmd),
        }
    }
}
