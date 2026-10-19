//! The supervisor: owns projects, sessions and status, and emits events.

use crate::config::ResolvedCommand;
use crate::pidfile;
use crate::project::{self, Project};
use crate::session::{CommandKey, Exit, Session, SessionObserver};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant, SystemTime};

/// Lifecycle state of a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Stopped,
    Starting,
    Running,
    /// Exit code 0: shown as done.
    Exited,
    /// Non-zero exit or signal. `restart_in` is set while a crash restart is pending.
    Crashed,
    /// Could not start (missing folder, PTY failure).
    Error,
}

/// Everything the UI needs to draw a command's status.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandState {
    pub status: Status,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
    pub signal: Option<String>,
    pub started_at: Option<SystemTime>,
    pub ended_at: Option<SystemTime>,
    /// Human-readable detail: error message, "restarting in 2 s", "gave up after 5 crashes".
    pub message: Option<String>,
    /// The session was started with a different run/cwd/env than the config now has.
    pub needs_restart: bool,
    /// When a crash restart is scheduled (backoff), the moment it will fire.
    pub restart_at: Option<SystemTime>,
    /// Port the running program announced it is listening on, if it printed one.
    pub port: Option<u16>,
    /// URL to open for that port.
    pub url: Option<String>,
}

impl Default for CommandState {
    fn default() -> Self {
        CommandState {
            status: Status::Stopped,
            pid: None,
            exit_code: None,
            signal: None,
            started_at: None,
            ended_at: None,
            message: None,
            needs_restart: false,
            restart_at: None,
            port: None,
            url: None,
        }
    }
}

impl CommandState {
    pub fn is_alive(&self) -> bool {
        matches!(self.status, Status::Starting | Status::Running)
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    /// Projects were added, removed or re-read from disk.
    ProjectsChanged,
    Status { key: CommandKey, state: CommandState },
    Output { key: CommandKey, bytes: Vec<u8> },
}

pub type Listener = Arc<dyn Fn(&Event) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    UnknownProject(String),
    ProjectInvalid(String),
    UnknownCommand(String),
    UnknownGroup(String),
    NotRunning(String),
    Spawn(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::UnknownProject(p) => write!(f, "unknown project {p:?}"),
            EngineError::ProjectInvalid(p) => write!(f, "project {p:?} has an invalid config"),
            EngineError::UnknownCommand(c) => write!(f, "unknown command {c:?}"),
            EngineError::UnknownGroup(g) => write!(f, "unknown group {g:?}"),
            EngineError::NotRunning(c) => write!(f, "{c:?} is not running"),
            EngineError::Spawn(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for EngineError {}

struct CrashRecord {
    times: Vec<Instant>,
    attempt: u32,
}

struct State {
    projects: Vec<Project>,
    sessions: HashMap<CommandKey, Arc<Session>>,
    states: HashMap<CommandKey, CommandState>,
    crashes: HashMap<CommandKey, CrashRecord>,
    /// Bumped by stop(); a pending crash restart with an older token is dropped.
    restart_tokens: HashMap<CommandKey, u64>,
    next_token: u64,
    default_size: (u16, u16),
}

pub struct Engine {
    projects_dir: PathBuf,
    pidfile_path: PathBuf,
    /// Last project.yaml we wrote per slug, to tell local edits from repo changes.
    last_written: Mutex<HashMap<String, crate::fragment::ProjectSeed>>,
    state: Mutex<State>,
    listeners: Mutex<Vec<(u64, Listener)>>,
    next_listener: Mutex<u64>,
    watcher: Mutex<Option<crate::watcher::Watcher>>,
    self_ref: Mutex<Weak<Engine>>,
}

impl Engine {
    /// Create an engine over `projects_dir` and load its projects. Starts nothing.
    pub fn new(projects_dir: impl Into<PathBuf>) -> Arc<Engine> {
        let projects_dir = projects_dir.into();
        let projects = Vec::new();
        let pidfile_path = project::app_support_dir().join(pidfile::FILE_NAME);
        let engine = Arc::new(Engine {
            projects_dir,
            pidfile_path,
            last_written: Mutex::new(HashMap::new()),
            state: Mutex::new(State {
                projects,
                sessions: HashMap::new(),
                states: HashMap::new(),
                crashes: HashMap::new(),
                restart_tokens: HashMap::new(),
                next_token: 1,
                default_size: (120, 30),
            }),
            listeners: Mutex::new(Vec::new()),
            next_listener: Mutex::new(1),
            watcher: Mutex::new(None),
            self_ref: Mutex::new(Weak::new()),
        });
        *engine.self_ref.lock().unwrap() = Arc::downgrade(&engine);
        engine.reload();
        engine
    }

    /// Engine over the default Projects directory.
    pub fn open_default() -> Arc<Engine> {
        let dir = project::default_projects_dir();
        let _ = std::fs::create_dir_all(&dir);
        Engine::new(dir)
    }

    pub fn projects_dir(&self) -> &Path {
        &self.projects_dir
    }

    fn me(&self) -> Arc<Engine> {
        self.self_ref.lock().unwrap().upgrade().expect("engine alive")
    }

    // ----- listeners -------------------------------------------------------

    pub fn subscribe(&self, listener: Listener) -> u64 {
        let mut next = self.next_listener.lock().unwrap();
        let id = *next;
        *next += 1;
        self.listeners.lock().unwrap().push((id, listener));
        id
    }

    pub fn unsubscribe(&self, id: u64) {
        self.listeners.lock().unwrap().retain(|(i, _)| *i != id);
    }

    fn emit(&self, event: Event) {
        let listeners: Vec<Listener> = self
            .listeners
            .lock()
            .unwrap()
            .iter()
            .map(|(_, l)| Arc::clone(l))
            .collect();
        for l in listeners {
            l(&event);
        }
    }

    // ----- projects --------------------------------------------------------

    /// Re-read the Projects directory. Never touches running sessions.
    pub fn reload(&self) {
        let mut projects = project::load_projects(&self.projects_dir);
        self.sync_seeds(&mut projects);
        self.watch_sources(&projects);
        let changed_keys: Vec<(CommandKey, CommandState)>;
        {
            let mut st = self.state.lock().unwrap();
            st.projects = projects;
            // Recompute "restart to apply" flags against the new configs.
            let mut changed = Vec::new();
            let flags: Vec<(CommandKey, bool)> = st
                .sessions
                .iter()
                .map(|(key, session)| {
                    let needs = Self::lookup_in(&st.projects, key)
                        .map(|cmd| !session.spec.matches(&cmd))
                        .unwrap_or(false);
                    (key.clone(), needs)
                })
                .collect();
            for (key, needs) in flags {
                if let Some(state) = st.states.get_mut(&key) {
                    if state.needs_restart != needs && state.is_alive() {
                        state.needs_restart = needs;
                        changed.push((key.clone(), state.clone()));
                    }
                }
            }
            changed_keys = changed;
        }
        self.emit(Event::ProjectsChanged);
        for (key, state) in changed_keys {
            self.emit(Event::Status { key, state });
        }
    }

    /// Start watching the Projects directory (and every project's fragment and seed
    /// folders); edits reload within about a second.
    pub fn watch(&self) -> Result<(), String> {
        let me = self.me();
        let weak = Arc::downgrade(&me);
        let watcher = crate::watcher::Watcher::new(&self.projects_dir, move || {
            if let Some(engine) = weak.upgrade() {
                engine.reload();
            }
        })?;
        *self.watcher.lock().unwrap() = Some(watcher);
        let projects = self.projects();
        self.watch_sources(&projects);
        Ok(())
    }

    /// Watch each project's fragment folders and its home `.switchboard` folder.
    fn watch_sources(&self, projects: &[Project]) {
        let mut guard = self.watcher.lock().unwrap();
        let Some(w) = guard.as_mut() else { return };
        for p in projects {
            for src in &p.sources {
                if let Some(dir) = src.parent() { w.add_path(dir); }
            }
            if let Some(home) = p.home() {
                w.add_path(&home.join(crate::fragment::DOT_DIR));
            }
        }
    }

    // ----- project.yaml write-back ----------------------------------------

    /// Keep each project's repo seed in step with its config, or flag the project when the
    /// seed changed under us (a pull): if the seed matches what we last wrote, the local
    /// config wins and is written out; otherwise the repo copy is newer and the user is
    /// asked before it is applied.
    fn sync_seeds(&self, projects: &mut [Project]) {
        use crate::fragment::ProjectSeed;
        let mut last = self.last_written.lock().unwrap();
        for p in projects.iter_mut() {
            let (Ok(cfg), Some(home)) = (&p.config, p.home()) else { continue };
            let current = ProjectSeed::from_config(cfg, &home);
            match ProjectSeed::load(&home) {
                Ok(on_disk) => {
                    if on_disk.same_settings(&current, &home) {
                        p.repo_settings_changed = false;
                    } else {
                        // Which side changed? What we last wrote decides; without that
                        // record (fresh launch), the newer file wins.
                        let local_is_newer = match last.get(&p.slug) {
                            Some(l) => on_disk.same_settings(l, &home),
                            None => {
                                let config_mtime = std::fs::metadata(p.config_path())
                                    .and_then(|m| m.modified())
                                    .ok()
                                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs_f64())
                                    .unwrap_or(0.0);
                                config_mtime > on_disk.updated_secs_pub(&crate::fragment::ProjectSeed::path_in(&home))
                            }
                        };
                        if local_is_newer {
                            let _ = current.save(&home, p.icon_path.as_deref());
                            p.repo_settings_changed = false;
                        } else {
                            p.repo_settings_changed = true;
                            continue;
                        }
                    }
                }
                Err(_) => {
                    let _ = current.save(&home, p.icon_path.as_deref());
                    p.repo_settings_changed = false;
                }
            }
            last.insert(p.slug.clone(), current);
        }
    }

    /// Make `folder` the project's home: set `home:` in config.yaml and write the seed there.
    pub fn adopt_home(&self, slug: &str, folder: &Path) -> Result<(), EngineError> {
        let p = self.project(slug).ok_or_else(|| EngineError::UnknownProject(slug.to_string()))?;
        let mut cfg = p.config.clone().map_err(|_| EngineError::ProjectInvalid(slug.to_string()))?;
        cfg.home = Some(crate::config::collapse_tilde(folder));
        p.save_config(&cfg).map_err(|e| EngineError::Spawn(e.to_string()))?;
        let seed = crate::fragment::ProjectSeed::from_config(&cfg, folder);
        seed.save(folder, p.icon_path.as_deref()).map_err(|e| EngineError::Spawn(e.to_string()))?;
        self.last_written.lock().unwrap().insert(slug.to_string(), seed);
        self.reload();
        Ok(())
    }

    /// Replace config.yaml with the repo's project.yaml (after "Settings changed in the repo").
    pub fn apply_repo_settings(&self, slug: &str) -> Result<(), EngineError> {
        let p = self.project(slug).ok_or_else(|| EngineError::UnknownProject(slug.to_string()))?;
        let home = p.home().ok_or_else(|| EngineError::Spawn("project has no home folder".into()))?;
        let seed = crate::fragment::ProjectSeed::load(&home).map_err(|e| EngineError::Spawn(e.to_string()))?;
        let cfg = seed.to_config(&home);
        p.save_config(&cfg).map_err(|e| EngineError::Spawn(e.to_string()))?;
        let icon = home.join(crate::fragment::DOT_DIR).join(crate::fragment::ICON_FILE);
        if icon.is_file() {
            let _ = std::fs::copy(&icon, p.dir.join(project::ICON_FILE));
        }
        self.last_written.lock().unwrap().insert(slug.to_string(), seed);
        self.reload();
        Ok(())
    }

    // ----- editing: order and icons -----------------------------------------

    fn edit_config(&self, slug: &str, f: impl FnOnce(&mut crate::config::ProjectConfig, &Project) -> Result<(), EngineError>) -> Result<(), EngineError> {
        let p = self.project(slug).ok_or_else(|| EngineError::UnknownProject(slug.to_string()))?;
        let mut cfg = p.config.clone().map_err(|_| EngineError::ProjectInvalid(slug.to_string()))?;
        f(&mut cfg, &p)?;
        cfg.validate().map_err(|e| EngineError::Spawn(e.to_string()))?;
        p.save_config(&cfg).map_err(|e| EngineError::Spawn(e.to_string()))?;
        self.reload();
        Ok(())
    }

    /// Move a group to `to_index` in the sidebar order.
    pub fn move_group(&self, slug: &str, group_id: &str, to_index: usize) -> Result<(), EngineError> {
        self.edit_config(slug, |cfg, _| {
            let from = cfg.groups.iter().position(|g| g.id == group_id).ok_or_else(|| EngineError::UnknownGroup(group_id.to_string()))?;
            let g = cfg.groups.remove(from);
            cfg.groups.insert(to_index.min(cfg.groups.len()), g);
            Ok(())
        })
    }

    /// Set or clear (None = derive from context) a group's icon.
    pub fn set_group_icon(&self, slug: &str, group_id: &str, icon: Option<&str>) -> Result<(), EngineError> {
        self.edit_config(slug, |cfg, _| {
            let g = cfg.groups.iter_mut().find(|g| g.id == group_id).ok_or_else(|| EngineError::UnknownGroup(group_id.to_string()))?;
            g.icon = icon.map(str::to_string);
            Ok(())
        })
    }

    /// Move a command to `to_index` among its siblings (same group, or the ungrouped list).
    /// Commands that come from a repo fragment are reordered in that fragment.
    pub fn move_command(&self, slug: &str, address: &str, to_index: usize) -> Result<(), EngineError> {
        fn shift(list: &mut Vec<crate::config::CommandConfig>, id: &str, to: usize) -> bool {
            let Some(from) = list.iter().position(|c| c.id == id) else { return false };
            let c = list.remove(from);
            list.insert(to.min(list.len()), c);
            true
        }
        let p = self.project(slug).ok_or_else(|| EngineError::UnknownProject(slug.to_string()))?;
        let cfg = p.config.clone().map_err(|_| EngineError::ProjectInvalid(slug.to_string()))?;
        let (group_id, cmd_id) = match address.split_once('/') {
            Some((g, c)) => (Some(g.to_string()), c.to_string()),
            None => (None, address.to_string()),
        };
        // A fragment-backed command: reorder inside the fragment file.
        if let Some(gid) = &group_id {
            let group = cfg.group(gid).ok_or_else(|| EngineError::UnknownGroup(gid.clone()))?;
            if let Some(from) = &group.from {
                if !group.commands.iter().any(|c| c.id == cmd_id) {
                    let folder = crate::fragment::resolve_folder(from, &crate::fragment::from_base(&cfg, &p.dir));
                    let mut fragment = crate::fragment::Fragment::load(&folder).map_err(|e| EngineError::Spawn(e.to_string()))?;
                    if !shift(&mut fragment.commands, &cmd_id, to_index) {
                        return Err(EngineError::UnknownCommand(address.to_string()));
                    }
                    fragment.save(&folder).map_err(|e| EngineError::Spawn(e.to_string()))?;
                    self.reload();
                    return Ok(());
                }
            }
        }
        self.edit_config(slug, |cfg, _| {
            let list = match &group_id {
                Some(gid) => &mut cfg.groups.iter_mut().find(|g| &g.id == gid).ok_or_else(|| EngineError::UnknownGroup(gid.clone()))?.commands,
                None => &mut cfg.commands,
            };
            if shift(list, &cmd_id, to_index) { Ok(()) } else { Err(EngineError::UnknownCommand(address.to_string())) }
        })
    }

    // ----- ad-hoc terminals ------------------------------------------------

    /// Open a new terminal entry in a group (or ungrouped) and start its shell.
    /// Returns the terminal's address.
    pub fn add_terminal(&self, slug: &str, group: Option<&str>) -> Result<String, EngineError> {
        use crate::terminals::{self, TerminalEntry};
        let p = self.project(slug).ok_or_else(|| EngineError::UnknownProject(slug.to_string()))?;
        let cfg = p.effective.as_ref().map_err(|_| EngineError::ProjectInvalid(slug.to_string()))?;
        let cwd = match group {
            Some(g) => {
                let gc = cfg.group(g).ok_or_else(|| EngineError::UnknownGroup(g.to_string()))?;
                gc.cwd.clone()
            }
            None => cfg.home.clone(),
        };
        let mut entries = p.terminals.clone();
        let entry = TerminalEntry {
            id: terminals::new_id(),
            name: terminals::next_name(&entries, group),
            group: group.map(str::to_string),
            cwd,
        };
        let address = entry.address();
        entries.push(entry);
        terminals::save(&p.dir, &entries).map_err(|e| EngineError::Spawn(e.to_string()))?;
        self.reload();
        self.start(&CommandKey::new(slug, &address))?;
        Ok(address)
    }

    /// Stop and forget a terminal.
    pub fn remove_terminal(&self, slug: &str, address: &str) -> Result<(), EngineError> {
        let p = self.project(slug).ok_or_else(|| EngineError::UnknownProject(slug.to_string()))?;
        let id = crate::terminals::id_from_address(address).ok_or_else(|| EngineError::UnknownCommand(address.to_string()))?;
        self.stop_blocking(&CommandKey::new(slug, address));
        let entries: Vec<_> = p.terminals.iter().filter(|t| t.id != id).cloned().collect();
        crate::terminals::save(&p.dir, &entries).map_err(|e| EngineError::Spawn(e.to_string()))?;
        self.reload();
        Ok(())
    }

    pub fn rename_terminal(&self, slug: &str, address: &str, name: &str) -> Result<(), EngineError> {
        let p = self.project(slug).ok_or_else(|| EngineError::UnknownProject(slug.to_string()))?;
        let id = crate::terminals::id_from_address(address).ok_or_else(|| EngineError::UnknownCommand(address.to_string()))?;
        let mut entries = p.terminals.clone();
        let Some(t) = entries.iter_mut().find(|t| t.id == id) else { return Err(EngineError::UnknownCommand(address.to_string())) };
        t.name = name.trim().to_string();
        crate::terminals::save(&p.dir, &entries).map_err(|e| EngineError::Spawn(e.to_string()))?;
        self.reload();
        Ok(())
    }

    /// Turn a terminal into a configured command in its group. Writes to the group's
    /// fragment when the group comes from one, else to config.yaml. The terminal is removed.
    pub fn promote_terminal(&self, slug: &str, address: &str, command_id: &str, name: &str, run: &str) -> Result<String, EngineError> {
        use crate::config::CommandConfig;
        let p = self.project(slug).ok_or_else(|| EngineError::UnknownProject(slug.to_string()))?;
        let id = crate::terminals::id_from_address(address).ok_or_else(|| EngineError::UnknownCommand(address.to_string()))?;
        let entry = p.terminals.iter().find(|t| t.id == id).cloned().ok_or_else(|| EngineError::UnknownCommand(address.to_string()))?;
        let mut cfg = p.config.clone().map_err(|_| EngineError::ProjectInvalid(slug.to_string()))?;
        let command_id = crate::project::slugify(command_id);
        let new_cmd = CommandConfig {
            id: command_id.clone(),
            name: Some(name.to_string()),
            run: run.to_string(),
            cwd: None,
            env: Default::default(),
            autostart: Some(false),
            restart_on_crash: None,
            terminal: false,
            extra: Default::default(),
        };
        let new_address = match &entry.group {
            Some(g) => {
                let base = crate::fragment::from_base(&cfg, &p.dir);
                let group = cfg.groups.iter_mut().find(|x| &x.id == g).ok_or_else(|| EngineError::UnknownGroup(g.clone()))?;
                if let Some(from) = group.from.clone() {
                    let folder = crate::fragment::resolve_folder(&from, &base);
                    let mut fragment = crate::fragment::Fragment::load(&folder).unwrap_or_default();
                    if fragment.commands.iter().any(|c| c.id == command_id) {
                        return Err(EngineError::Spawn(format!("a command with id {command_id:?} already exists in that group")));
                    }
                    fragment.commands.push(new_cmd);
                    fragment.save(&folder).map_err(|e| EngineError::Spawn(e.to_string()))?;
                } else {
                    if group.commands.iter().any(|c| c.id == command_id) {
                        return Err(EngineError::Spawn(format!("a command with id {command_id:?} already exists in that group")));
                    }
                    group.commands.push(new_cmd);
                    p.save_config(&cfg).map_err(|e| EngineError::Spawn(e.to_string()))?;
                }
                format!("{g}/{command_id}")
            }
            None => {
                if cfg.commands.iter().any(|c| c.id == command_id) {
                    return Err(EngineError::Spawn(format!("a command with id {command_id:?} already exists")));
                }
                cfg.commands.push(new_cmd);
                p.save_config(&cfg).map_err(|e| EngineError::Spawn(e.to_string()))?;
                command_id.clone()
            }
        };
        self.remove_terminal(slug, address)?;
        Ok(new_address)
    }

    /// Add a configured command to a project. With `group`, it joins that group (written to
    /// the group's fragment when the group comes from one); a group that does not exist is
    /// created with `new_group` (name, cwd). Without `group` it is ungrouped. Returns the
    /// command's address. Validates before writing; running sessions are untouched.
    pub fn add_command(&self, slug: &str, group: Option<&str>, new_group: Option<(Option<String>, Option<String>)>, cmd: crate::config::CommandConfig) -> Result<String, EngineError> {
        use crate::config::GroupConfig;
        let p = self.project(slug).ok_or_else(|| EngineError::UnknownProject(slug.to_string()))?;
        let mut cfg = p.config.clone().map_err(|_| EngineError::ProjectInvalid(slug.to_string()))?;
        let id = cmd.id.clone();
        let exists = |list: &[crate::config::CommandConfig]| list.iter().any(|c| c.id == id);
        let Some(g) = group else {
            if exists(&cfg.commands) {
                return Err(EngineError::Spawn(format!("{slug} already has a command {id:?}")));
            }
            cfg.commands.push(cmd);
            cfg.validate().map_err(|e| EngineError::Spawn(e.to_string()))?;
            p.save_config(&cfg).map_err(|e| EngineError::Spawn(e.to_string()))?;
            self.reload();
            return Ok(id);
        };
        let address = format!("{g}/{id}");
        let base = crate::fragment::from_base(&cfg, &p.dir);
        match cfg.groups.iter_mut().find(|x| x.id == g) {
            Some(group) => {
                if let Some(from) = group.from.clone() {
                    let folder = crate::fragment::resolve_folder(&from, &base);
                    let mut fragment = crate::fragment::Fragment::load(&folder).unwrap_or_default();
                    if exists(&fragment.commands) || exists(&group.commands) {
                        return Err(EngineError::Spawn(format!("group {g:?} already has a command {id:?}")));
                    }
                    fragment.commands.push(cmd);
                    fragment.save(&folder).map_err(|e| EngineError::Spawn(e.to_string()))?;
                    self.reload();
                    return Ok(address);
                }
                if exists(&group.commands) {
                    return Err(EngineError::Spawn(format!("group {g:?} already has a command {id:?}")));
                }
                group.commands.push(cmd);
            }
            None => {
                let (name, cwd) = new_group.ok_or_else(|| EngineError::UnknownGroup(g.to_string()))?;
                let mut group: GroupConfig = serde_yaml::from_str(&format!("id: {g:?}")).map_err(|e| EngineError::Spawn(e.to_string()))?;
                group.name = name;
                group.cwd = cwd;
                group.commands.push(cmd);
                cfg.groups.push(group);
            }
        }
        cfg.validate().map_err(|e| EngineError::Spawn(e.to_string()))?;
        p.save_config(&cfg).map_err(|e| EngineError::Spawn(e.to_string()))?;
        self.reload();
        Ok(address)
    }

    pub fn projects(&self) -> Vec<Project> {
        self.state.lock().unwrap().projects.clone()
    }

    pub fn project(&self, slug: &str) -> Option<Project> {
        self.state
            .lock()
            .unwrap()
            .projects
            .iter()
            .find(|p| p.slug == slug)
            .cloned()
    }

    fn lookup_in(projects: &[Project], key: &CommandKey) -> Option<ResolvedCommand> {
        projects
            .iter()
            .find(|p| p.slug == key.project)
            .and_then(|p| p.effective.as_ref().ok())
            .and_then(|c| c.resolve(&key.address))
    }

    fn lookup(&self, key: &CommandKey) -> Result<ResolvedCommand, EngineError> {
        let st = self.state.lock().unwrap();
        let project = st
            .projects
            .iter()
            .find(|p| p.slug == key.project)
            .ok_or_else(|| EngineError::UnknownProject(key.project.clone()))?;
        let config = project
            .effective
            .as_ref()
            .map_err(|_| EngineError::ProjectInvalid(key.project.clone()))?;
        config
            .resolve(&key.address)
            .ok_or_else(|| EngineError::UnknownCommand(key.address.clone()))
    }

    // ----- status ----------------------------------------------------------

    pub fn state_of(&self, key: &CommandKey) -> CommandState {
        self.state
            .lock()
            .unwrap()
            .states
            .get(key)
            .cloned()
            .unwrap_or_default()
    }

    /// All known command states (commands never started are absent).
    pub fn states(&self) -> HashMap<CommandKey, CommandState> {
        self.state.lock().unwrap().states.clone()
    }

    pub fn session(&self, key: &CommandKey) -> Option<Arc<Session>> {
        self.state.lock().unwrap().sessions.get(key).cloned()
    }

    /// Live sessions (alive or not yet reaped).
    pub fn sessions(&self) -> Vec<Arc<Session>> {
        self.state.lock().unwrap().sessions.values().cloned().collect()
    }

    fn set_state(&self, key: &CommandKey, f: impl FnOnce(&mut CommandState)) {
        let state = {
            let mut st = self.state.lock().unwrap();
            let entry = st.states.entry(key.clone()).or_default();
            f(entry);
            entry.clone()
        };
        self.emit(Event::Status { key: key.clone(), state });
    }

    /// Default PTY size for sessions started before a view attaches.
    pub fn set_default_size(&self, cols: u16, rows: u16) {
        self.state.lock().unwrap().default_size = (cols, rows);
    }

    // ----- start / stop / restart -----------------------------------------

    /// Start one command. A no-op if it is already alive.
    pub fn start(&self, key: &CommandKey) -> Result<(), EngineError> {
        let cmd = self.lookup(key)?;
        let size = {
            let mut st = self.state.lock().unwrap();
            if let Some(s) = st.sessions.get(key) {
                if !s.has_exited() {
                    return Ok(());
                }
            }
            // A manual start supersedes a pending crash restart ("Start now").
            let token = st.next_token;
            st.next_token += 1;
            st.restart_tokens.insert(key.clone(), token);
            st.default_size
        };
        self.set_state(key, |s| {
            *s = CommandState { status: Status::Starting, ..CommandState::default() };
        });
        let observer: Arc<dyn SessionObserver> = Arc::new(Observer { engine: Arc::downgrade(&self.me()) });
        match Session::spawn(key.clone(), &cmd, size, observer) {
            Ok(session) => {
                let pid = session.pid;
                let started_at = session.started_at;
                {
                    let mut st = self.state.lock().unwrap();
                    st.sessions.insert(key.clone(), session);
                }
                self.set_state(key, |s| {
                    s.status = Status::Starting;
                    s.pid = Some(pid);
                    s.started_at = Some(started_at);
                    s.message = None;
                });
                self.write_pidfile();
                Ok(())
            }
            Err(e) => {
                let msg = e.to_string();
                self.set_state(key, |s| {
                    s.status = Status::Error;
                    s.message = Some(msg.clone());
                    s.ended_at = Some(SystemTime::now());
                });
                Err(EngineError::Spawn(msg))
            }
        }
    }

    /// Stop one command in the background (SIGINT → SIGTERM → SIGKILL).
    pub fn stop(&self, key: &CommandKey) {
        let session = self.begin_stop(key);
        if let Some(session) = session {
            std::thread::spawn(move || session.stop_blocking());
        }
    }

    /// Stop one command and wait until it has exited.
    pub fn stop_blocking(&self, key: &CommandKey) {
        if let Some(session) = self.begin_stop(key) {
            session.stop_blocking();
        }
    }

    fn begin_stop(&self, key: &CommandKey) -> Option<Arc<Session>> {
        let mut st = self.state.lock().unwrap();
        // Cancel any pending crash restart.
        let token = st.next_token;
        st.next_token += 1;
        st.restart_tokens.insert(key.clone(), token);
        st.crashes.remove(key);
        let session = st.sessions.get(key).cloned();
        match session {
            Some(s) if !s.has_exited() => {
                s.request_stop();
                Some(s)
            }
            _ => {
                // Nothing alive; make sure a "restarting in…" message goes away.
                if let Some(state) = st.states.get_mut(key) {
                    if state.status == Status::Crashed && state.message.is_some() {
                        state.message = None;
                        state.restart_at = None;
                        let state = state.clone();
                        drop(st);
                        self.emit(Event::Status { key: key.clone(), state });
                    }
                }
                None
            }
        }
    }

    /// Stop (waiting) then start, in the background.
    pub fn restart(&self, key: &CommandKey) {
        let me = self.me();
        let key = key.clone();
        std::thread::spawn(move || {
            me.stop_blocking(&key);
            let _ = me.start(&key);
        });
    }

    /// Commands of a group, or every command when `group` is `None`.
    fn commands_of(&self, project: &str, group: Option<&str>) -> Result<Vec<ResolvedCommand>, EngineError> {
        let st = self.state.lock().unwrap();
        let p = st
            .projects
            .iter()
            .find(|p| p.slug == project)
            .ok_or_else(|| EngineError::UnknownProject(project.to_string()))?;
        let cfg = p
            .effective
            .as_ref()
            .map_err(|_| EngineError::ProjectInvalid(project.to_string()))?;
        match group {
            None => Ok(cfg.resolve_commands()),
            Some(g) => {
                if cfg.group(g).is_none() {
                    return Err(EngineError::UnknownGroup(g.to_string()));
                }
                Ok(cfg.resolve_group(g))
            }
        }
    }

    /// Start every autostart command in a project (or one group of it).
    pub fn start_all(&self, project: &str, group: Option<&str>) -> Result<Vec<CommandKey>, EngineError> {
        let mut started = Vec::new();
        for cmd in self.commands_of(project, group)? {
            if cmd.autostart {
                let key = CommandKey::new(project, &cmd.address);
                let _ = self.start(&key); // spawn errors land in the command's state
                started.push(key);
            }
        }
        Ok(started)
    }

    /// Stop every alive command in a project (or group), in the background.
    pub fn stop_all(&self, project: &str, group: Option<&str>) -> Result<(), EngineError> {
        for cmd in self.commands_of(project, group)? {
            self.stop(&CommandKey::new(project, &cmd.address));
        }
        Ok(())
    }

    /// Restart every alive command (and start autostart ones) in a project or group.
    pub fn restart_all(&self, project: &str, group: Option<&str>) -> Result<(), EngineError> {
        let cmds = self.commands_of(project, group)?;
        let me = self.me();
        let project = project.to_string();
        std::thread::spawn(move || {
            let keys: Vec<(CommandKey, bool)> = cmds
                .iter()
                .map(|c| {
                    let key = CommandKey::new(&project, &c.address);
                    let alive = me.state_of(&key).is_alive();
                    (key, alive || c.autostart)
                })
                .collect();
            let handles: Vec<_> = keys
                .iter()
                .map(|(k, _)| {
                    let me = Arc::clone(&me);
                    let k = k.clone();
                    std::thread::spawn(move || me.stop_blocking(&k))
                })
                .collect();
            for h in handles {
                let _ = h.join();
            }
            for (k, start) in keys {
                if start {
                    let _ = me.start(&k);
                }
            }
        });
        Ok(())
    }

    /// Stop every session everywhere and wait for all of them. Used on quit and Ctrl+C.
    pub fn stop_everything_blocking(&self) {
        let sessions: Vec<Arc<Session>> = {
            let mut st = self.state.lock().unwrap();
            st.crashes.clear();
            let token = st.next_token;
            st.next_token += 1;
            let keys: Vec<CommandKey> = st.sessions.keys().cloned().collect();
            for k in keys {
                st.restart_tokens.insert(k, token);
            }
            st.sessions.values().filter(|s| !s.has_exited()).cloned().collect()
        };
        let handles: Vec<_> = sessions
            .into_iter()
            .map(|s| std::thread::spawn(move || s.stop_blocking()))
            .collect();
        for h in handles {
            let _ = h.join();
        }
        self.write_pidfile();
    }

    pub fn any_alive(&self) -> bool {
        self.state.lock().unwrap().sessions.values().any(|s| !s.has_exited())
    }

    // ----- terminal I/O ----------------------------------------------------

    pub fn write_input(&self, key: &CommandKey, bytes: &[u8]) -> Result<(), EngineError> {
        let session = self
            .session(key)
            .filter(|s| !s.has_exited())
            .ok_or_else(|| EngineError::NotRunning(key.address.clone()))?;
        session
            .write_input(bytes)
            .map_err(|e| EngineError::NotRunning(e.to_string()))
    }

    pub fn resize(&self, key: &CommandKey, cols: u16, rows: u16) {
        if let Some(s) = self.session(key) {
            s.resize(cols, rows);
        }
    }

    /// Bytes to replay into a freshly attached terminal view.
    pub fn raw_tail(&self, key: &CommandKey) -> Vec<u8> {
        self.session(key).map(|s| s.raw_tail()).unwrap_or_default()
    }

    pub fn screen_text(&self, key: &CommandKey) -> String {
        self.session(key).map(|s| s.screen_text()).unwrap_or_default()
    }

    /// The last `lines` lines of a command's output as plain text, scrollback included.
    pub fn log_text(&self, key: &CommandKey, lines: usize) -> String {
        self.session(key).map(|s| s.log_text(lines)).unwrap_or_default()
    }

    // ----- session callbacks ----------------------------------------------

    fn on_running(&self, key: &CommandKey) {
        let mut st = self.state.lock().unwrap();
        let Some(state) = st.states.get_mut(key) else { return };
        if state.status != Status::Starting {
            return;
        }
        state.status = Status::Running;
        let state = state.clone();
        drop(st);
        self.emit(Event::Status { key: key.clone(), state });
    }

    fn on_exit(&self, key: &CommandKey, exit: Exit) {
        let (cmd, stop_requested, uptime) = {
            let st = self.state.lock().unwrap();
            let Some(session) = st.sessions.get(key) else { return };
            (
                Self::lookup_in(&st.projects, key),
                session.stop_requested(),
                session.uptime(),
            )
        };
        let crash = exit.is_crash();
        let restart_wanted = crash
            && !stop_requested
            && cmd.as_ref().map(|c| c.restart_on_crash).unwrap_or(false);

        let mut restart_delay: Option<Duration> = None;
        let mut gave_up = false;
        if restart_wanted {
            let mut st = self.state.lock().unwrap();
            let rec = st
                .crashes
                .entry(key.clone())
                .or_insert(CrashRecord { times: Vec::new(), attempt: 0 });
            let now = Instant::now();
            rec.times.push(now);
            rec.times.retain(|t| now.duration_since(*t) <= Duration::from_secs(120));
            if rec.times.len() > 5 {
                gave_up = true;
                st.crashes.remove(key);
            } else {
                if uptime >= Duration::from_secs(60) {
                    rec.attempt = 0;
                }
                let delay = Duration::from_secs((1u64 << rec.attempt.min(5)).min(30));
                rec.attempt += 1;
                restart_delay = Some(delay);
            }
        }

        let delay_msg = restart_delay.map(|d| format!("crashed, restarting in {} s", d.as_secs()));
        let restart_at = restart_delay.map(|d| SystemTime::now() + d);
        self.set_state(key, |s| {
            s.restart_at = restart_at;
            // A stop the user asked for is "stopped", however the process chose to die.
            s.status = if stop_requested {
                Status::Stopped
            } else if crash {
                Status::Crashed
            } else {
                Status::Exited
            };
            s.exit_code = exit.code();
            s.signal = match &exit {
                Exit::Signal(sig) => Some(sig.clone()),
                _ => None,
            };
            s.ended_at = Some(SystemTime::now());
            s.needs_restart = false;
            // Nothing is listening once the process is gone.
            s.port = None;
            s.url = None;
            s.message = if gave_up {
                Some("gave up after 5 crashes in 2 minutes".to_string())
            } else {
                delay_msg.clone()
            };
        });
        self.write_pidfile();

        if let Some(delay) = restart_delay {
            let token = {
                let mut st = self.state.lock().unwrap();
                let token = st.next_token;
                st.next_token += 1;
                st.restart_tokens.insert(key.clone(), token);
                token
            };
            let me = self.me();
            let key = key.clone();
            std::thread::spawn(move || {
                std::thread::sleep(delay);
                let still_valid = me.state.lock().unwrap().restart_tokens.get(&key) == Some(&token);
                if still_valid {
                    let _ = me.start(&key);
                }
            });
        }
    }

    // ----- pid file --------------------------------------------------------

    fn write_pidfile(&self) {
        let entries: Vec<pidfile::Entry> = {
            let st = self.state.lock().unwrap();
            st.sessions
                .values()
                .filter(|s| !s.has_exited())
                .map(|s| pidfile::Entry {
                    project: s.key.project.clone(),
                    address: s.key.address.clone(),
                    pid: s.pid,
                    started_at: s
                        .started_at
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                })
                .collect()
        };
        let _ = pidfile::write(&self.pidfile_path(), &entries);
    }

    /// `sessions.json` in the app support folder (shared by the app and the CLI).
    pub fn pidfile_path(&self) -> PathBuf {
        self.pidfile_path.clone()
    }

    /// Sessions recorded by a previous run whose process groups are still alive.
    pub fn leftover_sessions(&self) -> Vec<pidfile::Entry> {
        let own: std::collections::HashSet<u32> =
            self.sessions().iter().map(|s| s.pid).collect();
        pidfile::read(&self.pidfile_path())
            .into_iter()
            .filter(|e| !own.contains(&e.pid) && crate::session::pgid_alive(e.pid))
            .collect()
    }

    /// Kill leftover process groups from a previous run and clear them from the pid file.
    pub fn kill_leftovers(&self) -> usize {
        let leftovers = self.leftover_sessions();
        let n = leftovers.len();
        for e in &leftovers {
            pidfile::kill_group(e.pid);
        }
        self.write_pidfile();
        n
    }
}

struct Observer {
    engine: Weak<Engine>,
}

impl SessionObserver for Observer {
    fn on_output(&self, key: &CommandKey, bytes: &[u8]) {
        if let Some(e) = self.engine.upgrade() {
            e.emit(Event::Output { key: key.clone(), bytes: bytes.to_vec() });
        }
    }
    fn on_running(&self, key: &CommandKey) {
        if let Some(e) = self.engine.upgrade() {
            e.on_running(key);
        }
    }
    fn on_exit(&self, key: &CommandKey, exit: Exit) {
        if let Some(e) = self.engine.upgrade() {
            e.on_exit(key, exit);
        }
    }
    fn on_listening(&self, key: &CommandKey, listening: crate::ports::Listening) {
        if let Some(e) = self.engine.upgrade() {
            // Only while the session is alive; a late line must not revive a stopped row.
            if e.state_of(key).is_alive() {
                e.set_state(key, |s| {
                    s.port = Some(listening.port);
                    s.url = Some(listening.url.clone());
                });
            }
        }
    }
}
