//! Project folders under the Projects directory: loading, installing, slugs.

use crate::config::{ConfigError, ProjectConfig};
use std::fmt;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE: &str = "config.yaml";
pub const ICON_FILE: &str = "icon.png";

/// Default Projects directory: `~/Library/Application Support/Switchboard/Projects`.
pub fn default_projects_dir() -> PathBuf {
    app_support_dir().join("Projects")
}

/// `~/Library/Application Support/Switchboard`.
pub fn app_support_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SWITCHBOARD_HOME") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    let base = dirs::data_dir() // ~/Library/Application Support on macOS
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));
    base.join("Switchboard")
}

/// A project folder on disk. The config may have failed to load; the folder still shows up.
#[derive(Debug, Clone)]
pub struct Project {
    /// Folder name; also the stable identifier.
    pub slug: String,
    pub dir: PathBuf,
    pub config: Result<ProjectConfig, ConfigError>,
    pub icon_path: Option<PathBuf>,
}

impl Project {
    pub fn load(dir: &Path) -> Project {
        let slug = dir
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let config_path = dir.join(CONFIG_FILE);
        let config = if config_path.is_file() {
            ProjectConfig::load(&config_path)
        } else {
            Err(ConfigError::Io(format!("{} is missing", CONFIG_FILE)))
        };
        let icon_path = resolve_icon(dir, config.as_ref().ok());
        Project { slug, dir: dir.to_path_buf(), config, icon_path }
    }

    /// Display name: the config's name, or the folder name when the config is broken.
    pub fn name(&self) -> &str {
        match &self.config {
            Ok(c) => &c.name,
            Err(_) => &self.slug,
        }
    }

    pub fn config_path(&self) -> PathBuf {
        self.dir.join(CONFIG_FILE)
    }
}

fn resolve_icon(dir: &Path, config: Option<&ProjectConfig>) -> Option<PathBuf> {
    if let Some(icon) = config.and_then(|c| c.icon.as_deref()) {
        let p = crate::config::expand_tilde(icon);
        let p = if p.is_absolute() { p } else { dir.join(p) };
        if p.is_file() {
            return Some(p);
        }
    }
    let fallback = dir.join(ICON_FILE);
    fallback.is_file().then_some(fallback)
}

/// Load every project folder in `projects_dir`, sorted by folder name.
/// Files and hidden entries are skipped. A missing directory yields an empty list.
pub fn load_projects(projects_dir: &Path) -> Vec<Project> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(projects_dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || !path.is_dir() {
            continue;
        }
        out.push(Project::load(&path));
    }
    out.sort_by(|a, b| a.slug.to_lowercase().cmp(&b.slug.to_lowercase()));
    out
}

/// Folder-safe slug of a project name: lowercase ASCII letters, digits and dashes.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true;
    for ch in name.trim().chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "project".to_string() } else { out }
}

/// What to do when a project folder with the same slug already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conflict {
    /// Return `InstallError::Exists` and copy nothing.
    Fail,
    /// Delete the existing folder first.
    Replace,
    /// Install as `slug-2`, `slug-3`, …
    KeepBoth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    /// The source has no `config.yaml`.
    NoConfig(PathBuf),
    /// The config does not parse or validate; nothing was copied.
    Invalid(ConfigError),
    /// A project folder with this slug exists and `Conflict::Fail` was chosen.
    Exists(String),
    Io(String),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstallError::NoConfig(p) => write!(f, "{} has no {CONFIG_FILE}", p.display()),
            InstallError::Invalid(e) => write!(f, "{e}"),
            InstallError::Exists(s) => write!(f, "a project folder named {s:?} already exists"),
            InstallError::Io(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for InstallError {}

/// Validate `src` (a folder holding `config.yaml`, or the file itself) and return its config.
pub fn validate_source(src: &Path) -> Result<(PathBuf, ProjectConfig), InstallError> {
    let dir = if src.is_file() {
        src.parent().map(Path::to_path_buf).unwrap_or_default()
    } else {
        src.to_path_buf()
    };
    let config_path = dir.join(CONFIG_FILE);
    if !config_path.is_file() {
        return Err(InstallError::NoConfig(src.to_path_buf()));
    }
    let cfg = ProjectConfig::load(&config_path).map_err(InstallError::Invalid)?;
    Ok((dir, cfg))
}

/// Which slug an install of `src` would use, and whether it collides.
pub fn install_target(src: &Path, projects_dir: &Path) -> Result<(String, bool), InstallError> {
    let (_, cfg) = validate_source(src)?;
    let slug = slugify(&cfg.name);
    Ok((slug.clone(), projects_dir.join(&slug).exists()))
}

/// Validate and copy a project folder into `projects_dir`. Returns the installed project.
pub fn install_project(
    src: &Path,
    projects_dir: &Path,
    conflict: Conflict,
) -> Result<Project, InstallError> {
    let (src_dir, cfg) = validate_source(src)?;
    let base = slugify(&cfg.name);
    std::fs::create_dir_all(projects_dir).map_err(|e| InstallError::Io(e.to_string()))?;

    let mut dest = projects_dir.join(&base);
    if dest.exists() {
        match conflict {
            Conflict::Fail => return Err(InstallError::Exists(base)),
            Conflict::Replace => {
                std::fs::remove_dir_all(&dest).map_err(|e| InstallError::Io(e.to_string()))?;
            }
            Conflict::KeepBoth => {
                let mut n = 2;
                loop {
                    dest = projects_dir.join(format!("{base}-{n}"));
                    if !dest.exists() {
                        break;
                    }
                    n += 1;
                }
            }
        }
    }
    // Copy into a staging folder first so a half-copied project never shows up.
    let staging = projects_dir.join(format!(
        ".{}.installing",
        dest.file_name().unwrap().to_string_lossy()
    ));
    let _ = std::fs::remove_dir_all(&staging);
    copy_dir(&src_dir, &staging).map_err(|e| InstallError::Io(e.to_string()))?;
    std::fs::rename(&staging, &dest).map_err(|e| InstallError::Io(e.to_string()))?;
    Ok(Project::load(&dest))
}

fn copy_dir(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_string_lossy() == ".DS_Store" {
            continue;
        }
        let from = entry.path();
        let to = dest.join(&name);
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Remove a project folder.
pub fn delete_project(projects_dir: &Path, slug: &str) -> std::io::Result<()> {
    let dir = projects_dir.join(slug);
    if dir.is_dir() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}
