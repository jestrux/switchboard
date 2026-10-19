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
    /// Port the program announced it is listening on, and a URL to open.
    pub port: Option<u16>,
    pub url: Option<String>,
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
            port: s.port,
            url: s.url,
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
    /// An ad-hoc terminal (interactive shell) rather than a configured command.
    pub is_terminal: bool,
    pub state: CommandState,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct GroupInfo {
    pub id: String,
    pub name: String,
    pub cwd: Option<String>,
    /// The repo folder whose `.switchboard/commands.yaml` supplies this group, if any.
    pub from: Option<String>,
    /// Why the fragment could not be read (missing folder, bad YAML), if it couldn't.
    pub problem: Option<String>,
    /// Icon key to draw: the explicit one, else derived from the group's name and commands.
    pub icon: String,
    /// True when `icon:` is set in the config rather than derived.
    pub icon_is_explicit: bool,
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
    /// Base theme for its own window (`graphite`, `patch-bay`, `paper`), if pinned.
    pub theme: Option<String>,
    /// Custom theme as JSON (`{name?, base?, dark?: {token: "#hex"}, light?: {...}}`), if any.
    pub theme_custom: Option<String>,
    /// `system`, `light` or `dark` for its own window, if pinned.
    pub appearance: Option<String>,
    /// Repo folder whose `.switchboard/project.yaml` mirrors this project, if adopted.
    pub home: Option<String>,
    /// Folder to offer as the home when none is set: the first group's folder.
    pub suggested_home: Option<String>,
    /// The repo's project.yaml changed under us and is waiting to be applied.
    pub repo_settings_changed: bool,
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

/// A `.switchboard/project.yaml` found for a dropped folder.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SeedInfo {
    /// Folder holding the seed; becomes the project's home.
    pub home: String,
    pub name: String,
    pub slug: String,
    /// A project with this slug is already installed.
    pub exists: bool,
    /// Seconds since the epoch (the seed's `updated` stamp, else the file's mtime).
    pub updated_at: f64,
    pub icon_path: Option<String>,
    pub members: Vec<SeedMember>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SeedMember {
    pub group: String,
    pub folder: String,
    pub exists: bool,
    /// Commands the folder's fragment declares (0 when missing).
    pub command_count: u32,
}

/// What converting a folder's `solo.yml` would produce.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SoloPreview {
    pub name: String,
    pub slug: String,
    pub exists: bool,
    pub groups: Vec<SoloGroup>,
    pub notes: Vec<String>,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SoloGroup {
    pub name: String,
    pub folder: String,
    pub commands: Vec<String>,
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

/// Implemented in Swift: what the `switchboard` command asks of the app's windows.
/// Called from a socket thread.
#[uniffi::export(with_foreign)]
pub trait ControlHandler: Send + Sync {
    /// Bring the app forward, showing `project` when one is named.
    fn open_project(&self, project: Option<String>);
}

struct ControlBridge(Arc<dyn ControlHandler>);

impl sb_core::control::ControlHandler for ControlBridge {
    fn open_project(&self, project: Option<String>) {
        self.0.open_project(project);
    }
}

/// What "Install switchboard Command" did.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ToolsInstalled {
    pub command_path: String,
    /// The folder is on the login shell's PATH.
    pub on_path: bool,
    pub skill_path: Option<String>,
    pub skill_error: Option<String>,
    /// `~/.claude/CLAUDE.md` when the skill was added to it or was already listed there.
    pub claude_md_path: Option<String>,
    /// True when this install added the skill to that file.
    pub claude_md_added: bool,
    pub claude_md_error: Option<String>,
}

#[derive(uniffi::Object)]
pub struct Engine {
    inner: Arc<sb_core::Engine>,
    subscriptions: Mutex<Vec<u64>>,
    control: Mutex<Option<sb_core::control::ControlServer>>,
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
        is_terminal: c.terminal,
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
        theme_custom: None,
        appearance: None,
        home: None,
        suggested_home: None,
        repo_settings_changed: project.repo_settings_changed,
        error: None,
        commands: vec![],
        groups: vec![],
    };
    match &project.effective {
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
                    from: g.from.clone(),
                    problem: project.problems.iter().find(|p| p.group == g.id).map(|p| p.message.clone()),
                    icon: sb_core::icons::group_icon(
                        g.icon.as_deref(),
                        &g.id,
                        g.display_name(),
                        &g.commands.iter().filter(|c| !c.terminal).map(|c| c.run.as_str()).collect::<Vec<_>>(),
                    )
                    .to_string(),
                    icon_is_explicit: g.icon.is_some(),
                    commands: all
                        .iter()
                        .filter(|c| c.group.as_ref().map(|gr| gr.id.as_str()) == Some(g.id.as_str()))
                        .map(to_info)
                        .collect(),
                })
                .collect();
            ProjectInfo {
                color: cfg.color.clone(),
                theme: cfg.theme.as_ref().map(|t| t.base().to_string()),
                theme_custom: match &cfg.theme {
                    Some(sb_core::config::ThemeSpec::Custom(c)) => serde_json::to_string(c).ok(),
                    _ => None,
                },
                appearance: cfg.appearance.clone(),
                home: cfg.home.clone(),
                suggested_home: cfg
                    .groups
                    .iter()
                    .find_map(|g| g.cwd.clone())
                    .filter(|c| sb_core::config::expand_tilde(c).is_dir()),
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
        Arc::new(Engine { inner: sb_core::Engine::open_default(), subscriptions: Mutex::new(vec![]), control: Mutex::new(None) })
    }

    /// Engine over an explicit Projects directory (tests).
    #[uniffi::constructor]
    pub fn open(projects_dir: String) -> Arc<Self> {
        let _ = std::fs::create_dir_all(&projects_dir);
        Arc::new(Engine { inner: sb_core::Engine::new(PathBuf::from(projects_dir)), subscriptions: Mutex::new(vec![]), control: Mutex::new(None) })
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

    // ----- .switchboard seeds -----

    /// Project seeds reachable from a dropped folder, newest first. Empty when the
    /// folder has no `.switchboard/project.yaml`.
    pub fn discover_seeds(&self, folder: String) -> Vec<SeedInfo> {
        sb_core::fragment::discover_seeds(Path::new(&folder))
            .into_iter()
            .map(|c| {
                let slug = project::slugify(&c.seed.config.name);
                let icon = c.home.join(sb_core::fragment::DOT_DIR).join(sb_core::fragment::ICON_FILE);
                SeedInfo {
                    exists: self.inner.projects_dir().join(&slug).exists(),
                    slug,
                    name: c.seed.config.name.clone(),
                    home: c.home.display().to_string(),
                    updated_at: c.updated_at,
                    icon_path: icon.is_file().then(|| icon.display().to_string()),
                    members: c
                        .members
                        .iter()
                        .map(|(g, p, exists)| SeedMember {
                            group: g.clone(),
                            folder: sb_core::config::collapse_tilde(p),
                            exists: *exists,
                            command_count: sb_core::fragment::Fragment::load(p).map(|f| f.commands.len() as u32).unwrap_or(0),
                        })
                        .collect(),
                }
            })
            .collect()
    }

    /// Install the seed whose home is `home`. Returns the project's slug.
    pub fn install_seed(&self, home: String, conflict: ConflictPolicy) -> Result<String, SbError> {
        let conflict = match conflict {
            ConflictPolicy::Fail => CoreConflict::Fail,
            ConflictPolicy::Replace => CoreConflict::Replace,
            ConflictPolicy::KeepBoth => CoreConflict::KeepBoth,
        };
        let candidate = sb_core::fragment::discover_seeds(Path::new(&home))
            .into_iter()
            .find(|c| c.home == Path::new(&home).canonicalize().unwrap_or_else(|_| PathBuf::from(&home)))
            .ok_or_else(|| SbError::Message(format!("no .switchboard/project.yaml in {home}")))?;
        let dest = sb_core::fragment::install_seed(&candidate, self.inner.projects_dir(), conflict)?;
        self.inner.reload();
        Ok(dest.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
    }

    /// Keep this project's settings in `folder/.switchboard/project.yaml` from now on.
    pub fn adopt_home(&self, slug: String, folder: String) -> Result<(), SbError> {
        Ok(self.inner.adopt_home(&slug, Path::new(&folder))?)
    }

    /// Replace the local config with the repo's project.yaml.
    pub fn apply_repo_settings(&self, slug: String) -> Result<(), SbError> {
        Ok(self.inner.apply_repo_settings(&slug)?)
    }

    // ----- Solo import -----

    /// A preview of converting `folder`'s `solo.yml` (or its subfolders'), or None.
    pub fn preview_solo(&self, folder: String) -> Option<SoloPreview> {
        let files = sb_core::solo::find_solo_files(Path::new(&folder));
        let import = sb_core::solo::convert(&files).ok()?;
        let slug = project::slugify(&import.config.name);
        Some(SoloPreview {
            exists: self.inner.projects_dir().join(&slug).exists(),
            slug,
            name: import.config.name.clone(),
            groups: import
                .config
                .groups
                .iter()
                .map(|g| SoloGroup {
                    name: g.display_name().to_string(),
                    folder: g.cwd.clone().unwrap_or_default(),
                    commands: g.commands.iter().map(|c| c.display_name().to_string()).collect(),
                })
                .collect(),
            notes: import.notes,
            sources: import.sources.iter().map(|p| sb_core::config::collapse_tilde(p)).collect(),
        })
    }

    /// Convert and install `folder`'s Solo project. Returns the slug.
    pub fn install_solo(&self, folder: String, conflict: ConflictPolicy) -> Result<String, SbError> {
        let conflict = match conflict {
            ConflictPolicy::Fail => CoreConflict::Fail,
            ConflictPolicy::Replace => CoreConflict::Replace,
            ConflictPolicy::KeepBoth => CoreConflict::KeepBoth,
        };
        let files = sb_core::solo::find_solo_files(Path::new(&folder));
        let import = sb_core::solo::convert(&files).map_err(|e| SbError::Message(e.to_string()))?;
        let dest = sb_core::solo::install(&import, self.inner.projects_dir(), conflict)?;
        self.inner.reload();
        Ok(dest.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
    }

    // ----- order and icons -----

    pub fn move_group(&self, project: String, group: String, to_index: u32) -> Result<(), SbError> {
        Ok(self.inner.move_group(&project, &group, to_index as usize)?)
    }

    pub fn move_command(&self, project: String, address: String, to_index: u32) -> Result<(), SbError> {
        Ok(self.inner.move_command(&project, &address, to_index as usize)?)
    }

    /// Set a group's icon, or pass None to derive it from context again.
    pub fn set_group_icon(&self, project: String, group: String, icon: Option<String>) -> Result<(), SbError> {
        Ok(self.inner.set_group_icon(&project, &group, icon.as_deref())?)
    }

    // ----- ad-hoc terminals -----

    /// Open a terminal in a group's folder (or ungrouped). Returns its address.
    pub fn add_terminal(&self, project: String, group: Option<String>) -> Result<String, SbError> {
        Ok(self.inner.add_terminal(&project, group.as_deref())?)
    }

    pub fn remove_terminal(&self, project: String, address: String) -> Result<(), SbError> {
        Ok(self.inner.remove_terminal(&project, &address)?)
    }

    pub fn rename_terminal(&self, project: String, address: String, name: String) -> Result<(), SbError> {
        Ok(self.inner.rename_terminal(&project, &address, &name)?)
    }

    /// Turn a terminal into a configured command. Returns the new command's address.
    pub fn promote_terminal(&self, project: String, address: String, id: String, name: String, run: String) -> Result<String, SbError> {
        Ok(self.inner.promote_terminal(&project, &address, &id, &name, &run)?)
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

    /// Serve the control socket the `switchboard` command talks to. Fails when another
    /// Switchboard already serves it.
    pub fn serve_control(&self, handler: Arc<dyn ControlHandler>) -> Result<(), SbError> {
        let server = sb_core::control::serve(
            &self.inner,
            &sb_core::control::socket_path(),
            Some(Arc::new(ControlBridge(handler))),
        )
        .map_err(SbError::Message)?;
        *self.control.lock().unwrap() = Some(server);
        Ok(())
    }

    /// Stop serving and remove the socket (on quit).
    pub fn stop_control(&self) {
        self.control.lock().unwrap().take();
    }
}

/// Link the bundled `switchboard` command onto the PATH and the bundled agent skill into
/// `~/.claude/skills`. Both are symlinks into the app, so app updates refresh them.
#[uniffi::export]
pub fn install_tools(cli_path: String, skill_dir: String) -> Result<ToolsInstalled, SbError> {
    let home = dirs::home_dir().ok_or_else(|| SbError::Message("no home folder".into()))?;
    let got = sb_core::tools::install(Path::new(&cli_path), Path::new(&skill_dir), &home, None).map_err(SbError::Message)?;
    use sb_core::tools::ClaudeMd;
    let (claude_md_path, claude_md_added, claude_md_error) = match &got.claude_md {
        Ok(ClaudeMd::Added(p)) => (Some(p.display().to_string()), true, None),
        Ok(ClaudeMd::AlreadyListed(p)) => (Some(p.display().to_string()), false, None),
        Ok(ClaudeMd::NoFile) => (None, false, None),
        Err(e) => (None, false, Some(e.clone())),
    };
    Ok(ToolsInstalled {
        claude_md_path,
        claude_md_added,
        claude_md_error,
        command_path: got.command.display().to_string(),
        on_path: got.on_path,
        skill_path: got.skill.as_ref().ok().map(|p| p.display().to_string()),
        skill_error: got.skill.err(),
    })
}

/// The command and the skill are both installed, so the launch offer is not needed.
#[uniffi::export]
pub fn tools_installed() -> bool {
    dirs::home_dir().is_some_and(|h| sb_core::tools::is_installed(&h))
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

/// Icon keys a group may use, in picker order.
#[uniffi::export]
pub fn group_icon_keys() -> Vec<String> {
    sb_core::icons::GROUP_ICONS.iter().map(|s| s.to_string()).collect()
}

/// The user's login shell, as the engine will use it.
#[uniffi::export]
pub fn login_shell() -> String {
    sb_core::session::login_shell()
}
