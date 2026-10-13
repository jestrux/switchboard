//! Converting Solo's `solo.yml` into a Switchboard project.
//!
//! Solo is one project per folder with a flat map of processes. We keep every process as
//! a command and group them by working folder, since groups are how Switchboard gives
//! commands a shared folder.

use crate::config::{collapse_tilde, expand_tilde, CommandConfig, ConfigError, GroupConfig, ProjectConfig};
use crate::project::{slugify, Conflict, InstallError, CONFIG_FILE, ICON_FILE};
use serde::Deserialize;
use serde_yaml::{Mapping, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SOLO_FILE: &str = "solo.yml";

#[derive(Debug, Deserialize)]
struct SoloFile {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    /// Kept as a mapping so the file's order is the sidebar order.
    #[serde(default)]
    processes: Mapping,
}

#[derive(Debug, Deserialize)]
struct SoloProcess {
    command: String,
    #[serde(default)]
    working_dir: Option<String>,
    #[serde(default)]
    auto_start: Option<bool>,
    #[serde(default)]
    auto_restart: Option<bool>,
    #[serde(default)]
    restart_when_changed: Vec<Value>,
    #[serde(default)]
    env: BTreeMap<String, String>,
}

/// The result of converting one or more `solo.yml` files.
#[derive(Debug, Clone)]
pub struct SoloImport {
    pub config: ProjectConfig,
    /// A PNG to install as the project icon, if the Solo file named one that exists.
    pub icon: Option<PathBuf>,
    /// Things that did not carry over, for the user to read.
    pub notes: Vec<String>,
    /// The files that were read.
    pub sources: Vec<PathBuf>,
}

/// `solo.yml` in `folder`, else in its immediate subfolders.
pub fn find_solo_files(folder: &Path) -> Vec<PathBuf> {
    let direct = folder.join(SOLO_FILE);
    if direct.is_file() {
        return vec![direct];
    }
    let mut found = Vec::new();
    if let Ok(entries) = std::fs::read_dir(folder) {
        for entry in entries.flatten() {
            let p = entry.path().join(SOLO_FILE);
            if entry.path().is_dir() && p.is_file() {
                found.push(p);
            }
        }
    }
    found.sort();
    found
}

/// Convert the given `solo.yml` files into one project. The first file names the project;
/// processes from all of them are grouped by working folder.
pub fn convert(files: &[PathBuf]) -> Result<SoloImport, ConfigError> {
    if files.is_empty() {
        return Err(ConfigError::Io(format!("no {SOLO_FILE} found")));
    }
    let mut name: Option<String> = None;
    let mut icon: Option<PathBuf> = None;
    let mut notes = Vec::new();
    // folder -> (group, used command ids), in first-seen order.
    let mut groups: Vec<(PathBuf, GroupConfig)> = Vec::new();

    for file in files {
        let base = file.parent().map(Path::to_path_buf).unwrap_or_default();
        let text = std::fs::read_to_string(file)
            .map_err(|e| ConfigError::Io(format!("{}: {e}", collapse_tilde(file))))?;
        let solo: SoloFile = serde_yaml::from_str(&text)
            .map_err(|e| ConfigError::Parse(format!("{}: {e}", collapse_tilde(file))))?;
        if name.is_none() {
            name = solo.name.clone().or_else(|| base.file_name().map(|n| n.to_string_lossy().into_owned()));
        }
        if icon.is_none() {
            if let Some(raw) = &solo.icon {
                let p = expand_tilde(raw);
                let p = if p.is_absolute() { p } else { base.join(p) };
                if p.is_file() && p.extension().map(|e| e.eq_ignore_ascii_case("png")).unwrap_or(false) {
                    icon = Some(p);
                } else if p.is_file() {
                    notes.push(format!("icon {} is not a PNG; pick one later", collapse_tilde(&p)));
                } else {
                    notes.push(format!("icon {raw} was not found"));
                }
            }
        }
        for (key, value) in &solo.processes {
            let Some(key) = key.as_str() else { continue };
            let proc: SoloProcess = match serde_yaml::from_value(value.clone()) {
                Ok(p) => p,
                Err(e) => {
                    notes.push(format!("process {key:?} skipped: {e}"));
                    continue;
                }
            };
            if !proc.restart_when_changed.is_empty() {
                notes.push(format!("{key}: restart_when_changed has no equivalent and was dropped"));
            }
            let folder = match &proc.working_dir {
                Some(w) => {
                    let p = expand_tilde(w);
                    if p.is_absolute() { p } else { base.join(p) }
                }
                None => base.clone(),
            };
            let idx = match groups.iter().position(|(f, _)| *f == folder) {
                Some(i) => i,
                None => {
                    let label = folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "commands".into());
                    let mut id = slugify(&label);
                    let mut n = 2;
                    while groups.iter().any(|(_, g)| g.id == id) {
                        id = format!("{}-{n}", slugify(&label));
                        n += 1;
                    }
                    groups.push((folder.clone(), GroupConfig {
                        id,
                        name: Some(label),
                        cwd: Some(collapse_tilde(&folder)),
                        from: None,
                        overrides: Default::default(),
                        icon: None,
                        commands: Vec::new(),
                        extra: Default::default(),
                    }));
                    groups.len() - 1
                }
            };
            let group = &mut groups[idx].1;
            let mut id = slugify(key);
            let mut n = 2;
            while group.commands.iter().any(|c| c.id == id) {
                id = format!("{}-{n}", slugify(key));
                n += 1;
            }
            group.commands.push(CommandConfig {
                id,
                name: Some(key.to_string()),
                run: proc.command,
                cwd: None,
                env: proc.env,
                autostart: Some(proc.auto_start.unwrap_or(true)),
                restart_on_crash: proc.auto_restart.filter(|r| *r),
                terminal: false,
                extra: Default::default(),
            });
        }
    }

    let config = ProjectConfig {
        version: 1,
        name: name.unwrap_or_else(|| "Project".into()),
        icon: icon.as_ref().map(|_| ICON_FILE.to_string()),
        color: None,
        theme: None,
        appearance: None,
        home: None,
        commands: Vec::new(),
        groups: groups.into_iter().map(|(_, g)| g).collect(),
        extra: Default::default(),
    };
    config.validate()?;
    Ok(SoloImport { config, icon, notes, sources: files.to_vec() })
}

/// Write a converted project into the Projects directory. Returns its folder.
pub fn install(import: &SoloImport, projects_dir: &Path, conflict: Conflict) -> Result<PathBuf, InstallError> {
    let base = slugify(&import.config.name);
    std::fs::create_dir_all(projects_dir).map_err(|e| InstallError::Io(e.to_string()))?;
    let mut dest = projects_dir.join(&base);
    if dest.exists() {
        match conflict {
            Conflict::Fail => return Err(InstallError::Exists(base)),
            Conflict::Replace => std::fs::remove_dir_all(&dest).map_err(|e| InstallError::Io(e.to_string()))?,
            Conflict::KeepBoth => {
                let mut n = 2;
                loop {
                    dest = projects_dir.join(format!("{base}-{n}"));
                    if !dest.exists() { break; }
                    n += 1;
                }
            }
        }
    }
    std::fs::create_dir_all(&dest).map_err(|e| InstallError::Io(e.to_string()))?;
    let yaml = import.config.to_yaml().map_err(InstallError::Invalid)?;
    std::fs::write(dest.join(CONFIG_FILE), yaml).map_err(|e| InstallError::Io(e.to_string()))?;
    if let Some(icon) = &import.icon {
        std::fs::copy(icon, dest.join(ICON_FILE)).map_err(|e| InstallError::Io(e.to_string()))?;
    }
    Ok(dest)
}
