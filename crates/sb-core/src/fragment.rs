//! `.switchboard/` files inside repo folders: command fragments, the project seed, and
//! how a project config composes them.

use crate::config::{collapse_tilde, expand_tilde, CommandConfig, ConfigError, ProjectConfig};
use serde::{Deserialize, Serialize};
use serde_yaml::Mapping;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const DOT_DIR: &str = ".switchboard";
pub const FRAGMENT_FILE: &str = "commands.yaml";
pub const PROJECT_FILE: &str = "project.yaml";
pub const ICON_FILE: &str = "icon.png";

/// `<folder>/.switchboard/commands.yaml`: the commands that belong to one repo folder.
/// `cwd` defaults to the folder; a command's own `cwd` may be relative to it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Fragment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<CommandConfig>,
    #[serde(flatten)]
    pub extra: Mapping,
}

impl Fragment {
    pub fn path_in(folder: &Path) -> PathBuf {
        folder.join(DOT_DIR).join(FRAGMENT_FILE)
    }

    pub fn load(folder: &Path) -> Result<Fragment, ConfigError> {
        let path = Self::path_in(folder);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| ConfigError::Io(format!("{}: {e}", collapse_tilde(&path))))?;
        let f: Fragment = serde_yaml::from_str(&text)
            .map_err(|e| ConfigError::Parse(format!("{}: {e}", collapse_tilde(&path))))?;
        Ok(f)
    }

    pub fn save(&self, folder: &Path) -> Result<(), ConfigError> {
        let dir = folder.join(DOT_DIR);
        std::fs::create_dir_all(&dir).map_err(|e| ConfigError::Io(e.to_string()))?;
        let text = serde_yaml::to_string(self).map_err(|e| ConfigError::Parse(e.to_string()))?;
        std::fs::write(dir.join(FRAGMENT_FILE), text).map_err(|e| ConfigError::Io(e.to_string()))
    }
}

/// Per-command tweaks a project applies over a fragment's commands (`groups[].overrides`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CommandOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autostart: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart_on_crash: Option<bool>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// Hide this command from the project without editing the repo's fragment.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

/// Where a relative `from:` or `cwd` resolves from. The result is lexically normalized
/// (`a/b/../c` → `a/c`) so it reads cleanly and compares equal to the real folder.
pub fn resolve_folder(raw: &str, base: &Path) -> PathBuf {
    let p = expand_tilde(raw);
    normalize(&if p.is_absolute() { p } else { base.join(p) })
}

fn normalize(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Base for relative `from:` paths: the home folder when `home` is set (so `../api` is a
/// sibling repo), else `fallback`.
pub fn from_base(config: &ProjectConfig, fallback: &Path) -> PathBuf {
    match &config.home {
        Some(h) => expand_tilde(h),
        None => fallback.to_path_buf(),
    }
}

/// `abs` written relative to `home` when they share a parent (`.` for home itself,
/// `../name` for a sibling), else as a `~` path.
fn relative_to_home(abs: &Path, home: &Path) -> String {
    // Compare real paths so /var vs /private/var or a symlinked workspace still relates.
    let abs_c = abs.canonicalize().unwrap_or_else(|_| abs.to_path_buf());
    let home_c = home.canonicalize().unwrap_or_else(|_| home.to_path_buf());
    let (abs, home) = (abs_c.as_path(), home_c.as_path());
    if abs == home {
        return ".".to_string();
    }
    if let Some(parent) = home.parent() {
        if let Ok(rest) = abs.strip_prefix(parent) {
            return format!("../{}", rest.display());
        }
    }
    collapse_tilde(abs)
}

/// A problem with one group's fragment; the group still exists, just without those commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupProblem {
    pub group: String,
    pub message: String,
}

/// The outcome of composing a config: inline + fragment commands, plus what was read.
#[derive(Debug, Clone)]
pub struct Composed {
    /// The config with every `from:` group's commands expanded inline (cwd set to the
    /// fragment folder, overrides applied). `from`/`overrides` are kept for reference.
    pub config: ProjectConfig,
    pub problems: Vec<GroupProblem>,
    /// Fragment files that were read (or should exist), for the file watcher.
    pub sources: Vec<PathBuf>,
}

impl ProjectConfig {
    /// Expand `from:` groups using their folders' fragments. `base` resolves relative
    /// `from:` paths when `home` is unset (the folder holding this config).
    pub fn compose(&self, base: &Path) -> Composed {
        let mut out = self.clone();
        let mut problems = Vec::new();
        let mut sources = Vec::new();
        let from_base = from_base(self, base);
        for group in &mut out.groups {
            let Some(from) = &group.from else { continue };
            let folder = resolve_folder(from, &from_base);
            sources.push(Fragment::path_in(&folder));
            if group.cwd.is_none() {
                group.cwd = Some(collapse_tilde(&folder));
            }
            if !folder.is_dir() {
                problems.push(GroupProblem {
                    group: group.id.clone(),
                    message: format!("Folder not found: {}", collapse_tilde(&folder)),
                });
                continue;
            }
            match Fragment::load(&folder) {
                Ok(fragment) => {
                    if group.name.is_none() {
                        group.name = fragment.name.clone();
                    }
                    let mut merged: Vec<CommandConfig> = Vec::new();
                    for mut cmd in fragment.commands {
                        if let Some(o) = group.overrides.get(&cmd.id) {
                            if o.hidden {
                                continue;
                            }
                            if o.name.is_some() { cmd.name = o.name.clone(); }
                            if o.autostart.is_some() { cmd.autostart = o.autostart; }
                            if o.restart_on_crash.is_some() { cmd.restart_on_crash = o.restart_on_crash; }
                            for (k, v) in &o.env { cmd.env.insert(k.clone(), v.clone()); }
                        }
                        // A fragment's relative cwd is relative to its folder.
                        if let Some(c) = &cmd.cwd {
                            let p = expand_tilde(c);
                            if !p.is_absolute() {
                                cmd.cwd = Some(collapse_tilde(&folder.join(p)));
                            }
                        }
                        merged.push(cmd);
                    }
                    // Inline commands written in the project come after the fragment's.
                    merged.extend(group.commands.drain(..));
                    group.commands = merged;
                }
                Err(ConfigError::Io(_)) if !Fragment::path_in(&folder).exists() => {
                    problems.push(GroupProblem {
                        group: group.id.clone(),
                        message: format!("No {DOT_DIR}/{FRAGMENT_FILE} in {}", collapse_tilde(&folder)),
                    });
                }
                Err(e) => problems.push(GroupProblem { group: group.id.clone(), message: e.to_string() }),
            }
        }
        Composed { config: out, problems, sources }
    }
}

// ----- project.yaml: the seed kept in a repo ---------------------------------------

/// `<home>/.switchboard/project.yaml`: a project config as versioned in a repo. Same
/// schema as `config.yaml` minus `home`, plus `updated`; relative `from:` paths are
/// relative to the home folder (`../api` is a sibling repo).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectSeed {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    #[serde(flatten)]
    pub config: ProjectConfig,
}

/// A project.yaml found while importing, with where it came from.
#[derive(Debug, Clone)]
pub struct SeedCandidate {
    pub home: PathBuf,
    pub seed: ProjectSeed,
    /// Seconds since the epoch: the `updated` stamp, else the file's mtime.
    pub updated_at: f64,
    /// Member folders the seed references, resolved, and whether each exists.
    pub members: Vec<(String, PathBuf, bool)>,
}

impl ProjectSeed {
    pub fn path_in(home: &Path) -> PathBuf {
        home.join(DOT_DIR).join(PROJECT_FILE)
    }

    pub fn load(home: &Path) -> Result<ProjectSeed, ConfigError> {
        let path = Self::path_in(home);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| ConfigError::Io(format!("{}: {e}", collapse_tilde(&path))))?;
        let seed: ProjectSeed = serde_yaml::from_str(&text)
            .map_err(|e| ConfigError::Parse(format!("{}: {e}", collapse_tilde(&path))))?;
        seed.config.validate()?;
        Ok(seed)
    }

    pub fn updated_secs_pub(&self, path: &Path) -> f64 { self.updated_secs(path) }

    fn updated_secs(&self, path: &Path) -> f64 {
        if let Some(u) = &self.updated {
            if let Ok(t) = humantime_parse(u) {
                return t;
            }
        }
        std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }

    /// Member folders (every `from:` group) resolved relative to the home folder.
    pub fn members(&self, home: &Path) -> Vec<(String, PathBuf, bool)> {
        let base = home.to_path_buf();
        self.config
            .groups
            .iter()
            .filter_map(|g| g.from.as_ref().map(|f| (g.id.clone(), resolve_folder(f, &base))))
            .map(|(id, p)| { let exists = p.is_dir(); (id, p, exists) })
            .collect()
    }

    /// The seed turned into an installable `config.yaml`: absolute (`~`) paths, `home` set.
    pub fn to_config(&self, home: &Path) -> ProjectConfig {
        let base = home.to_path_buf();
        let mut cfg = self.config.clone();
        cfg.home = Some(collapse_tilde(home));
        for g in &mut cfg.groups {
            if let Some(f) = &g.from {
                g.from = Some(collapse_tilde(&resolve_folder(f, &base)));
            }
            if let Some(c) = &g.cwd {
                g.cwd = Some(collapse_tilde(&resolve_folder(c, &base)));
            }
        }
        if cfg.icon.is_some() {
            cfg.icon = Some(ICON_FILE.to_string());
        }
        cfg
    }

    /// Build the seed from an installed config: paths relative to the home folder where
    /// possible, `home` dropped, `updated` stamped now.
    pub fn from_config(config: &ProjectConfig, home: &Path) -> ProjectSeed {
        let mut cfg = config.clone();
        cfg.home = None;
        let rel = |raw: &str| -> String { relative_to_home(&expand_tilde(raw), home) };
        for g in &mut cfg.groups {
            if let Some(f) = &g.from { g.from = Some(rel(f)); }
            if let Some(c) = &g.cwd { g.cwd = Some(rel(c)); }
        }
        if cfg.icon.is_some() {
            cfg.icon = Some(ICON_FILE.to_string());
        }
        ProjectSeed { updated: Some(humantime_now()), config: cfg }
    }

    /// Equal apart from the `updated` stamp and from how paths are spelled: both seeds
    /// are resolved against `home` first, so `.` and `../app` match when they name the
    /// same folder.
    pub fn same_settings(&self, other: &ProjectSeed, home: &Path) -> bool {
        self.to_config(home) == other.to_config(home)
    }

    /// Write `project.yaml` (and the icon, if `icon_src` is given) into `home/.switchboard`.
    pub fn save(&self, home: &Path, icon_src: Option<&Path>) -> Result<(), ConfigError> {
        let dir = home.join(DOT_DIR);
        std::fs::create_dir_all(&dir).map_err(|e| ConfigError::Io(e.to_string()))?;
        let text = serde_yaml::to_string(self).map_err(|e| ConfigError::Parse(e.to_string()))?;
        std::fs::write(dir.join(PROJECT_FILE), text).map_err(|e| ConfigError::Io(e.to_string()))?;
        if let Some(src) = icon_src {
            let dest = dir.join(ICON_FILE);
            if src != dest {
                std::fs::copy(src, dest).map_err(|e| ConfigError::Io(e.to_string()))?;
            }
        }
        Ok(())
    }
}

/// Find project seeds for a dropped folder: the folder itself, and every member folder
/// its seed references (so drifted copies are all considered). Newest first.
pub fn discover_seeds(folder: &Path) -> Vec<SeedCandidate> {
    let mut found: Vec<SeedCandidate> = Vec::new();
    let mut queue = vec![folder.to_path_buf()];
    let mut seen = std::collections::HashSet::new();
    while let Some(dir) = queue.pop() {
        let Ok(canon) = dir.canonicalize() else { continue };
        if !seen.insert(canon.clone()) {
            continue;
        }
        let Ok(seed) = ProjectSeed::load(&canon) else { continue };
        let members = seed.members(&canon);
        for (_, p, exists) in &members {
            if *exists {
                queue.push(p.clone());
            }
        }
        let updated_at = seed.updated_secs(&ProjectSeed::path_in(&canon));
        found.push(SeedCandidate { home: canon, seed, updated_at, members });
    }
    found.sort_by(|a, b| b.updated_at.partial_cmp(&a.updated_at).unwrap_or(std::cmp::Ordering::Equal));
    found
}

/// Install a seed as `Projects/<slug>/config.yaml` (+ icon). Returns the installed folder.
pub fn install_seed(candidate: &SeedCandidate, projects_dir: &Path, conflict: crate::project::Conflict) -> Result<PathBuf, crate::project::InstallError> {
    use crate::project::{slugify, Conflict, InstallError};
    let cfg = candidate.seed.to_config(&candidate.home);
    let base = slugify(&cfg.name);
    std::fs::create_dir_all(projects_dir).map_err(|e| InstallError::Io(e.to_string()))?;
    let mut dest = projects_dir.join(&base);
    if dest.exists() {
        match conflict {
            Conflict::Fail => return Err(InstallError::Exists(base)),
            Conflict::Replace => { std::fs::remove_dir_all(&dest).map_err(|e| InstallError::Io(e.to_string()))?; }
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
    let yaml = cfg.to_yaml().map_err(InstallError::Invalid)?;
    std::fs::write(dest.join(crate::project::CONFIG_FILE), yaml).map_err(|e| InstallError::Io(e.to_string()))?;
    let icon = candidate.home.join(DOT_DIR).join(ICON_FILE);
    if icon.is_file() {
        std::fs::copy(&icon, dest.join(ICON_FILE)).map_err(|e| InstallError::Io(e.to_string()))?;
    }
    Ok(dest)
}

// ----- tiny RFC 3339 helpers (no chrono) -------------------------------------------

fn humantime_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_rfc3339(secs)
}

fn format_rfc3339(secs: u64) -> String {
    // Civil-from-days (Howard Hinnant's algorithm).
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mth = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mth <= 2 { y + 1 } else { y };
    format!("{y:04}-{mth:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

fn humantime_parse(s: &str) -> Result<f64, ()> {
    // YYYY-MM-DDTHH:MM:SSZ only (what we write).
    let b = s.as_bytes();
    if b.len() < 19 { return Err(()); }
    let num = |a: usize, l: usize| -> Result<i64, ()> { s.get(a..a + l).and_then(|t| t.parse().ok()).ok_or(()) };
    let (y, mo, d, h, mi, se) = (num(0, 4)?, num(5, 2)?, num(8, 2)?, num(11, 2)?, num(14, 2)?, num(17, 2)?);
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
    let yoe = y2 - era * 400;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Ok((days * 86400 + h * 3600 + mi * 60 + se) as f64)
}
