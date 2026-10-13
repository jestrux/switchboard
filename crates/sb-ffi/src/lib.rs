//! UniFFI surface over `sb-core` for the Mac app.
//!
//! Everything here is a thin translation layer: records mirror core types, and the
//! `Engine` object forwards to `sb_core::Engine`. No process logic lives in this crate.

uniffi::setup_scaffolding!();

use sb_core::engine::{Event, Status as CoreStatus};
use sb_core::project::{self, Conflict as CoreConflict};
use sb_core::session::CommandKey;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum CommandStatus {
    Stopped,
    Starting,
    Running,
    /// Exit code 0: shown as done.
    Exited,
    Crashed,
    /// Could not start (missing folder, PTY failure).
    Error,
}

impl From<CoreStatus> for CommandStatus {
    fn from(s: CoreStatus) -> Self {
        match s {
            CoreStatus::Stopped => CommandStatus::Stopped,
            CoreStatus::Starting => CommandStatus::Starting,
            CoreStatus::Running => CommandStatus::Running,
            CoreStatus::Exited => CommandStatus::Exited,
            CoreStatus::Crashed => CommandStatus::Crashed,
            CoreStatus::Error => CommandStatus::Error,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct CommandState {
    pub status: CommandStatus,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
    pub signal: Option<String>,
    /// Seconds since the Unix epoch.
    pub started_at: Option<f64>,
    pub ended_at: Option<f64>,
    pub message: Option<String>,
    pub needs_restart: bool,
    /// Seconds since the Unix epoch when a scheduled crash restart fires.
    pub restart_at: Option<f64>,
}

fn secs(t: Option<SystemTime>) -> Option<f64> {
    t.and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs_f64())
}

impl From<sb_core::CommandState> for CommandState {
    fn from(s: sb_core::CommandState) -> Self {
        CommandState {
            status: s.status.into(),
            pid: s.pid,
            exit_code: s.exit_code,
            signal: s.signal,
            started_at: secs(s.started_at),
            ended_at: secs(s.ended_at),
            message: s.message,
            needs_restart: s.needs_restart,
            restart_at: secs(s.restart_at),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct CommandInfo {
    /// `group/command`, or `command` when ungrouped.
    pub address: String,
    pub id: String,
    /// Own display name, e.g. `make:migrate`.
    pub name: String,
    /// Prefixed display name, e.g. `Backend / make:migrate`.
    pub display_name: String,
    pub group_id: Option<String>,
    pub run: String,
    /// The folder as written in the config (`~`-relative), if any.
    pub cwd: Option<String>,
    /// Expanded absolute folder, if any.
    pub cwd_path: Option<String>,
    /// False when a folder is set but does not exist on this machine.
    pub cwd_exists: bool,
    pub autostart: bool,
    pub restart_on_crash: bool,
    pub state: CommandState,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct GroupInfo {
    pub id: String,
    pub name: String,
    pub cwd: Option<String>,
    pub commands: Vec<CommandInfo>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProjectInfo {
    /// Folder name; stable identifier.
    pub slug: String,
    pub name: String,
    pub dir: String,
    pub config_path: String,
    pub icon_path: Option<String>,
    pub color: Option<String>,
    /// Per-project look for its own window (`graphite`, `patch-bay`, `paper`), if pinned.
    pub theme: Option<String>,
    /// `system`, `light` or `dark` for its own window, if pinned.
    pub appearance: Option<String>,
    /// Set when the config does not parse or validate; groups and commands are then empty.
    pub error: Option<String>,
    pub commands: Vec<CommandInfo>,
    pub groups: Vec<GroupInfo>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct LeftoverSession {
    pub project: String,
    pub address: String,
    pub pid: u32,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum ConflictPolicy {
    Fail,
    Replace,
    KeepBoth,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct InstallPreview {
    pub slug: String,
    pub name: String,
    pub exists: bool,
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum SbError {
    #[error("{0}")]
    Message(String),
}

impl From<sb_core::EngineError> for SbError {
    fn from(e: sb_core::EngineError) -> Self {
        SbError::Message(e.to_string())
    }
}
impl From<project::InstallError> for SbError {
    fn from(e: project::InstallError) -> Self {
        SbError::Message(e.to_string())
    }
}

/// Implemented in Swift; called from engine threads (never the main thread).
#[uniffi::export(with_foreign)]
pub trait EngineListener: Send + Sync {
    fn on_projects_changed(&self);
    fn on_status(&self, project: String, address: String, state: CommandState);
    fn on_output(&self, project: String, address: String, bytes: Vec<u8>);
}

#[derive(uniffi::Object)]
pub struct Engine {
    inner: Arc<sb_core::Engine>,
    subscriptions: Mutex<Vec<u64>>,
}

fn key(project: &str, address: &str) -> CommandKey {
    CommandKey::new(project, address)
}

fn describe(project: &project::Project, engine: &sb_core::Engine) -> ProjectInfo {
    let states = engine.states();
    let state_for = |address: &str| -> CommandState {
        states
            .get(&key(&project.slug, address))
            .cloned()
            .unwrap_or_default()
            .into()
    };
    let to_info = |c: &sb_core::ResolvedCommand| CommandInfo {
        address: c.address.clone(),
        id: c.id.clone(),
        name: c.name.clone(),
        display_name: c.display_name.clone(),
        group_id: c.group.as_ref().map(|g| g.id.clone()),
        run: c.run.clone(),
        cwd: c.cwd_raw.clone(),
        cwd_path: c.cwd.as_ref().map(|p| p.display().to_string()),
        cwd_exists: c.cwd.as_ref().map(|p| p.is_dir()).unwrap_or(true),
        autostart: c.autostart,
        restart_on_crash: c.restart_on_crash,
        state: state_for(&c.address),
    };
    let base = ProjectInfo {
        slug: project.slug.clone(),
        name: project.name().to_string(),
        dir: project.dir.display().to_string(),
        config_path: project.config_path().display().to_string(),
        icon_path: project.icon_path.as_ref().map(|p| p.display().to_string()),
        color: None,
        theme: None,
        appearance: None,
        error: None,
        commands: vec![],
        groups: vec![],
    };
    match &project.config {
        Err(e) => ProjectInfo { error: Some(e.to_string()), ..base },
        Ok(cfg) => {
            let all = cfg.resolve_commands();
            let commands = all.iter().filter(|c| c.group.is_none()).map(to_info).collect();
            let groups = cfg
                .groups
                .iter()
                .map(|g| GroupInfo {
                    id: g.id.clone(),
                    name: g.display_name().to_string(),
                    cwd: g.cwd.clone(),
                    commands: all
                        .iter()
                        .filter(|c| c.group.as_ref().map(|gr| gr.id.as_str()) == Some(g.id.as_str()))
                        .map(to_info)
                        .collect(),
                })
                .collect();
            ProjectInfo {
                color: cfg.color.clone(),
                theme: cfg.theme.clone(),
                appearance: cfg.appearance.clone(),
                commands,
                groups,
                ..base
            }
        }
    }
}

#[uniffi::export]
impl Engine {
    /// Engine over the default Projects directory (`~/Library/Application Support/Switchboard/Projects`).
    #[uniffi::constructor]
    pub fn open_default() -> Arc<Self> {
        Arc::new(Engine { inner: sb_core::Engine::open_default(), subscriptions: Mutex::new(vec![]) })
    }

    /// Engine over an explicit Projects directory (tests).
    #[uniffi::constructor]
    pub fn open(projects_dir: String) -> Arc<Self> {
        let _ = std::fs::create_dir_all(&projects_dir);
        Arc::new(Engine { inner: sb_core::Engine::new(PathBuf::from(projects_dir)), subscriptions: Mutex::new(vec![]) })
    }

    pub fn projects_dir(&self) -> String {
        self.inner.projects_dir().display().to_string()
    }

    /// Start the FSEvents watcher; `on_projects_changed` fires after edits settle.
    pub fn watch(&self) -> Result<(), SbError> {
        self.inner.watch().map_err(SbError::Message)
    }

    pub fn reload(&self) {
        self.inner.reload();
    }

    pub fn projects(&self) -> Vec<ProjectInfo> {
        self.inner.projects().iter().map(|p| describe(p, &self.inner)).collect()
    }

    pub fn project(&self, slug: String) -> Option<ProjectInfo> {
        self.inner.project(&slug).map(|p| describe(&p, &self.inner))
    }

    pub fn subscribe(&self, listener: Arc<dyn EngineListener>) -> u64 {
        let id = self.inner.subscribe(Arc::new(move |event: &Event| match event {
            Event::ProjectsChanged => listener.on_projects_changed(),
            Event::Status { key, state } => {
                listener.on_status(key.project.clone(), key.address.clone(), state.clone().into())
            }
            Event::Output { key, bytes } => {
                listener.on_output(key.project.clone(), key.address.clone(), bytes.clone())
            }
        }));
        self.subscriptions.lock().unwrap().push(id);
        id
    }

    pub fn unsubscribe(&self, id: u64) {
        self.inner.unsubscribe(id);
    }

    // ----- lifecycle -----

    pub fn start(&self, project: String, address: String) -> Result<(), SbError> {
        Ok(self.inner.start(&key(&project, &address))?)
    }

    pub fn stop(&self, project: String, address: String) {
        self.inner.stop(&key(&project, &address));
    }

    pub fn restart(&self, project: String, address: String) {
        self.inner.restart(&key(&project, &address));
    }

    /// Start every autostart command of a project, or of one group.
    pub fn start_all(&self, project: String, group: Option<String>) -> Result<(), SbError> {
        self.inner.start_all(&project, group.as_deref())?;
        Ok(())
    }

    pub fn stop_all(&self, project: String, group: Option<String>) -> Result<(), SbError> {
        Ok(self.inner.stop_all(&project, group.as_deref())?)
    }

    pub fn restart_all(&self, project: String, group: Option<String>) -> Result<(), SbError> {
        Ok(self.inner.restart_all(&project, group.as_deref())?)
    }

    /// Stop every session and block until all have exited. Call before quitting.
    pub fn stop_everything(&self) {
        self.inner.stop_everything_blocking();
    }

    pub fn any_alive(&self) -> bool {
        self.inner.any_alive()
    }

    pub fn state(&self, project: String, address: String) -> CommandState {
        self.inner.state_of(&key(&project, &address)).into()
    }

    // ----- terminal I/O -----

    pub fn write_input(&self, project: String, address: String, bytes: Vec<u8>) -> Result<(), SbError> {
        Ok(self.inner.write_input(&key(&project, &address), &bytes)?)
    }

    pub fn resize(&self, project: String, address: String, cols: u16, rows: u16) {
        self.inner.resize(&key(&project, &address), cols, rows);
    }

    /// PTY size used for sessions started before any view attaches.
    pub fn set_default_size(&self, cols: u16, rows: u16) {
        self.inner.set_default_size(cols, rows);
    }

    /// Bytes received so far (bounded), to replay into a newly attached terminal view.
    pub fn raw_tail(&self, project: String, address: String) -> Vec<u8> {
        self.inner.raw_tail(&key(&project, &address))
    }

    pub fn screen_text(&self, project: String, address: String) -> String {
        self.inner.screen_text(&key(&project, &address))
    }

    // ----- projects on disk -----

    /// Validate a dropped folder and report the slug it would install under.
    pub fn preview_install(&self, source: String) -> Result<InstallPreview, SbError> {
        let (dir, cfg) = project::validate_source(Path::new(&source))?;
        let _ = dir;
        let slug = project::slugify(&cfg.name);
        let exists = self.inner.projects_dir().join(&slug).exists();
        Ok(InstallPreview { slug, name: cfg.name, exists })
    }

    /// Validate and copy a project folder into the Projects directory. Returns its slug.
    pub fn install(&self, source: String, conflict: ConflictPolicy) -> Result<String, SbError> {
        let conflict = match conflict {
            ConflictPolicy::Fail => CoreConflict::Fail,
            ConflictPolicy::Replace => CoreConflict::Replace,
            ConflictPolicy::KeepBoth => CoreConflict::KeepBoth,
        };
        let p = project::install_project(Path::new(&source), self.inner.projects_dir(), conflict)?;
        self.inner.reload();
        Ok(p.slug)
    }

    pub fn delete_project(&self, slug: String) -> Result<(), SbError> {
        let _ = self.inner.stop_all(&slug, None);
        project::delete_project(self.inner.projects_dir(), &slug)
            .map_err(|e| SbError::Message(e.to_string()))?;
        self.inner.reload();
        Ok(())
    }

    /// Sessions a previous run left behind (their process groups are still alive).
    pub fn leftover_sessions(&self) -> Vec<LeftoverSession> {
        self.inner
            .leftover_sessions()
            .into_iter()
            .map(|e| LeftoverSession { project: e.project, address: e.address, pid: e.pid })
            .collect()
    }

    pub fn kill_leftovers(&self) -> u32 {
        self.inner.kill_leftovers() as u32
    }
}

/// Validate a `config.yaml` (or its folder) without an engine. Returns error lines.
#[uniffi::export]
pub fn validate_config(path: String) -> Vec<String> {
    match project::validate_source(Path::new(&path)) {
        Ok(_) => vec![],
        Err(project::InstallError::Invalid(e)) => e.messages(),
        Err(e) => vec![e.to_string()],
    }
}

/// The user's login shell, as the engine will use it.
#[uniffi::export]
pub fn login_shell() -> String {
    sb_core::session::login_shell()
}
