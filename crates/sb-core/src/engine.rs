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
        let projects = project::load_projects(&projects_dir);
        let engine = Arc::new(Engine {
            projects_dir,
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
        let projects = project::load_projects(&self.projects_dir);
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

    /// Start watching the Projects directory; edits reload within about a second.
    pub fn watch(&self) -> Result<(), String> {
        let me = self.me();
        let weak = Arc::downgrade(&me);
        let watcher = crate::watcher::Watcher::new(&self.projects_dir, move || {
            if let Some(engine) = weak.upgrade() {
                engine.reload();
            }
        })?;
        *self.watcher.lock().unwrap() = Some(watcher);
        Ok(())
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
            .and_then(|p| p.config.as_ref().ok())
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
            .config
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
            let st = self.state.lock().unwrap();
            if let Some(s) = st.sessions.get(key) {
                if !s.has_exited() {
                    return Ok(());
                }
            }
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
            .config
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
        self.set_state(key, |s| {
            s.status = if crash { Status::Crashed } else { Status::Exited };
            s.exit_code = exit.code();
            s.signal = match &exit {
                Exit::Signal(sig) => Some(sig.clone()),
                _ => None,
            };
            s.ended_at = Some(SystemTime::now());
            s.needs_restart = false;
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

    fn pidfile_path(&self) -> PathBuf {
        self.projects_dir
            .parent()
            .map(|p| p.join(pidfile::FILE_NAME))
            .unwrap_or_else(|| self.projects_dir.join(pidfile::FILE_NAME))
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
}
