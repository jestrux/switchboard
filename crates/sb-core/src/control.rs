//! The app's control socket. The running app serves it so the `switchboard` command (and
//! the agents that call it) can start, stop and read commands in the app's own terminals.
//!
//! One JSON request per line, one JSON reply per line. Every reply has `ok`; a failed one
//! has `error`. The socket lives in the app support folder and is private to the user.

use crate::config::ResolvedCommand;
use crate::engine::{CommandState, Engine, EngineError, Status};
use crate::project;
use crate::session::CommandKey;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, SystemTime};

pub const SOCKET_NAME: &str = "control.sock";

/// `control.sock` in the app support folder (moves with `SWITCHBOARD_HOME`).
pub fn socket_path() -> PathBuf {
    project::app_support_dir().join(SOCKET_NAME)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Ping,
    /// Every project, or one, with each command's live state.
    Status { project: Option<String> },
    /// No target: the autostart commands. A group: its autostart commands. A command: it.
    Start { project: String, target: Option<String> },
    /// No target: everything alive in the project. Waits until they have exited.
    Stop { project: String, target: Option<String> },
    /// Stops (waiting), then starts what was alive plus the autostart commands.
    Restart { project: String, target: Option<String> },
    /// The last `lines` lines of a command's output, as plain text.
    Logs { project: String, address: String, lines: usize },
    /// Bring the app forward, showing a project if one is named.
    Open { project: Option<String> },
    /// A project's home. No folder: its current home and a suggestion, changing nothing.
    /// A folder: keep the project's settings in that repo (`.switchboard/project.yaml`), as
    /// the app's Keep Settings in Repo does. Only on the author's say-so.
    Keep { project: String, folder: Option<String> },
    /// Stream one command's terminal. After the reply line the connection carries its output
    /// as raw bytes (server to client) and input and size frames (client to server; see
    /// `frame`), until either side closes it. The terminal is sized to `cols`×`rows` first.
    Attach { project: String, address: String, cols: u16, rows: u16 },
    /// A login shell in a group's folder (ungrouped: the project's home), started at once.
    /// Kept per machine in the project's `terminals.yaml`; replies with its address.
    NewTerminal { project: String, group: Option<String> },
    /// End a terminal's shell and forget it.
    CloseTerminal { project: String, address: String },
    RenameTerminal { project: String, address: String, name: String },
}

/// Client-to-server frames on an attached connection: one kind byte, a big-endian `u32`
/// length, then the payload.
pub const FRAME_INPUT: u8 = 0;
/// Payload: columns then rows, each a big-endian `u16`.
pub const FRAME_RESIZE: u8 = 1;

/// One frame, ready to write.
pub fn frame(kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(5 + payload.len());
    f.push(kind);
    f.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    f.extend_from_slice(payload);
    f
}

pub fn resize_frame(cols: u16, rows: u16) -> Vec<u8> {
    let mut p = cols.to_be_bytes().to_vec();
    p.extend_from_slice(&rows.to_be_bytes());
    frame(FRAME_RESIZE, &p)
}

/// What the app does with requests the engine cannot answer alone.
pub trait ControlHandler: Send + Sync {
    fn open_project(&self, project: Option<String>);
}

/// A served socket. Dropping it stops serving and removes the socket file.
pub struct ControlServer {
    path: PathBuf,
    stop: Arc<AtomicBool>,
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accept loop so it sees the flag.
        let _ = UnixStream::connect(&self.path);
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Serve `engine` on `path`. Fails if another live process already serves it; a socket
/// file left behind by a crash is replaced.
pub fn serve(engine: &Arc<Engine>, path: &Path, handler: Option<Arc<dyn ControlHandler>>) -> Result<ControlServer, String> {
    if path.exists() {
        if UnixStream::connect(path).is_ok() {
            return Err(format!("another Switchboard is already serving {}", path.display()));
        }
        let _ = std::fs::remove_file(path);
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let listener = UnixListener::bind(path).map_err(|e| format!("could not open {}: {e}", path.display()))?;
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    let stop = Arc::new(AtomicBool::new(false));
    let weak = Arc::downgrade(engine);
    let flag = Arc::clone(&stop);
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            if flag.load(Ordering::SeqCst) {
                break;
            }
            let Ok(conn) = conn else { continue };
            let (weak, handler) = (weak.clone(), handler.clone());
            std::thread::spawn(move || connection(conn, weak, handler));
        }
    });
    Ok(ControlServer { path: path.to_path_buf(), stop })
}

fn connection(conn: UnixStream, engine: Weak<Engine>, handler: Option<Arc<dyn ControlHandler>>) {
    let Ok(mut out) = conn.try_clone() else { return };
    let mut reader = BufReader::new(conn);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if line.trim().is_empty() {
            continue;
        }
        let reply = match (serde_json::from_str::<Request>(&line), engine.upgrade()) {
            (_, None) => fail("Switchboard is shutting down"),
            (Err(e), _) => fail(&format!("bad request: {e}")),
            // Streams until the client leaves; the connection is done after that.
            (Ok(Request::Attach { project, address, cols, rows }), Some(engine)) => {
                let weak = Arc::downgrade(&engine);
                drop(engine);
                attach(weak, &project, &address, (cols, rows), reader, out);
                return;
            }
            (Ok(req), Some(engine)) => handle(&engine, handler.as_deref(), req),
        };
        if writeln!(out, "{reply}").is_err() {
            break;
        }
    }
}

fn keep(engine: &Arc<Engine>, slug: &str, folder: Option<&str>) -> Result<Value, String> {
    let p = engine.project(slug).ok_or_else(|| EngineError::UnknownProject(slug.to_string()).to_string())?;
    let home = p.home();
    let suggestion = crate::home::suggest(&p);
    let Some(folder) = folder else {
        return Ok(json!({ "home": home, "suggestion": suggestion }));
    };
    let folder = crate::config::expand_tilde(folder);
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    let folder = std::fs::canonicalize(&folder).unwrap_or(folder);
    if let Some(h) = &home {
        let h = std::fs::canonicalize(h).unwrap_or_else(|_| h.clone());
        if h == folder {
            return Ok(json!({ "home": h, "kept": false }));
        }
        return Err(format!("{slug} already keeps its settings in {}; change it from the app (Keep Settings in Repo)", h.display()));
    }
    engine.adopt_home(slug, &folder).map_err(|e| e.to_string())?;
    Ok(json!({ "home": folder, "kept": true }))
}

fn fail(msg: &str) -> Value {
    json!({ "ok": false, "error": msg })
}

/// The project a request names, if any.
fn named_project(req: &Request) -> Option<&str> {
    match req {
        Request::Ping => None,
        Request::Status { project } | Request::Open { project } => project.as_deref(),
        Request::Start { project, .. }
        | Request::Stop { project, .. }
        | Request::Restart { project, .. }
        | Request::Logs { project, .. }
        | Request::Keep { project, .. }
        | Request::Attach { project, .. }
        | Request::NewTerminal { project, .. }
        | Request::CloseTerminal { project, .. }
        | Request::RenameTerminal { project, .. } => Some(project),
    }
}

fn handle(engine: &Arc<Engine>, handler: Option<&dyn ControlHandler>, req: Request) -> Value {
    // A project installed a moment ago (`switchboard import` then `start`) can beat the file
    // watcher's quiet period; read the Projects folder again before calling it unknown.
    if let Some(p) = named_project(&req) {
        if engine.project(p).is_none() {
            engine.reload();
        }
    }
    let result = match req {
        Request::Ping => Ok(json!({
            "app": "Switchboard",
            "pid": std::process::id(),
            "version": env!("CARGO_PKG_VERSION"),
            "projects_dir": engine.projects_dir(),
        })),
        Request::Status { project } => status(engine, project.as_deref()),
        Request::Start { project, target } => start(engine, &project, target.as_deref()),
        Request::Stop { project, target } => stop(engine, &project, target.as_deref()),
        Request::Restart { project, target } => restart(engine, &project, target.as_deref()),
        Request::Logs { project, address, lines } => logs(engine, &project, &address, lines),
        Request::Open { project } => {
            if let Some(p) = &project {
                if engine.project(p).is_none() {
                    return fail(&EngineError::UnknownProject(p.clone()).to_string());
                }
            }
            match handler {
                Some(h) => {
                    h.open_project(project);
                    Ok(json!({}))
                }
                None => Err("this Switchboard has no window to open".to_string()),
            }
        }
        Request::Keep { project, folder } => keep(engine, &project, folder.as_deref()),
        Request::Attach { .. } => Err("attach takes over its connection; it is not handled here".to_string()),
        Request::NewTerminal { project, group } => engine
            .add_terminal(&project, group.as_deref())
            .map(|address| json!({ "address": address }))
            .map_err(|e| e.to_string()),
        Request::CloseTerminal { project, address } => engine.remove_terminal(&project, &address).map(|_| json!({})).map_err(|e| e.to_string()),
        Request::RenameTerminal { project, address, name } => engine.rename_terminal(&project, &address, &name).map(|_| json!({})).map_err(|e| e.to_string()),
    };
    match result {
        Ok(Value::Object(mut map)) => {
            map.insert("ok".into(), Value::Bool(true));
            Value::Object(map)
        }
        Ok(other) => json!({ "ok": true, "data": other }),
        Err(e) => fail(&e),
    }
}

// ----- targets ---------------------------------------------------------------

/// The commands a target names. `everything` picks every command of the project or of a
/// named group (stop) rather than only the autostart ones (start).
fn commands_for(engine: &Engine, slug: &str, target: Option<&str>, everything: bool) -> Result<Vec<ResolvedCommand>, String> {
    let p = engine.project(slug).ok_or_else(|| EngineError::UnknownProject(slug.to_string()).to_string())?;
    let cfg = p.effective.as_ref().map_err(|e| format!("project {slug:?} has an invalid config:\n{e}"))?;
    let pick = |cmds: Vec<ResolvedCommand>| -> Vec<ResolvedCommand> {
        cmds.into_iter().filter(|c| everything || c.autostart).collect()
    };
    match target {
        None => Ok(pick(cfg.resolve_commands())),
        Some(t) => {
            if let Some(c) = cfg.resolve(t) {
                Ok(vec![c])
            } else if cfg.group(t).is_some() {
                Ok(pick(cfg.resolve_group(t)))
            } else {
                Err(format!("{slug} has no command or group named {t:?}"))
            }
        }
    }
}

fn start(engine: &Engine, slug: &str, target: Option<&str>) -> Result<Value, String> {
    let cmds = commands_for(engine, slug, target, false)?;
    if cmds.is_empty() {
        return Err("nothing to start (no autostart commands matched; name a command)".into());
    }
    let mut started = Vec::new();
    for c in &cmds {
        let key = CommandKey::new(slug, &c.address);
        // A spawn failure lands in the command's state, which the reply carries.
        let _ = engine.start(&key);
        started.push(command_json(c, &engine.state_of(&key)));
    }
    Ok(json!({ "commands": started }))
}

fn stop_keys(engine: &Arc<Engine>, keys: &[CommandKey]) {
    let handles: Vec<_> = keys
        .iter()
        .cloned()
        .map(|k| {
            let e = Arc::clone(engine);
            std::thread::spawn(move || e.stop_blocking(&k))
        })
        .collect();
    for h in handles {
        let _ = h.join();
    }
}

fn stop(engine: &Arc<Engine>, slug: &str, target: Option<&str>) -> Result<Value, String> {
    let cmds = commands_for(engine, slug, target, true)?;
    let alive: Vec<&ResolvedCommand> = cmds
        .iter()
        .filter(|c| engine.state_of(&CommandKey::new(slug, &c.address)).is_alive())
        .collect();
    let keys: Vec<CommandKey> = cmds.iter().map(|c| CommandKey::new(slug, &c.address)).collect();
    // Every matched key, so a pending crash restart is cancelled too.
    stop_keys(engine, &keys);
    let stopped: Vec<Value> = alive
        .iter()
        .map(|c| command_json(c, &engine.state_of(&CommandKey::new(slug, &c.address))))
        .collect();
    Ok(json!({ "commands": stopped }))
}

fn restart(engine: &Arc<Engine>, slug: &str, target: Option<&str>) -> Result<Value, String> {
    let cmds = commands_for(engine, slug, target, true)?;
    let single = matches!(target, Some(t) if cmds.len() == 1 && cmds[0].address == t);
    let chosen: Vec<&ResolvedCommand> = cmds
        .iter()
        .filter(|c| single || c.autostart || engine.state_of(&CommandKey::new(slug, &c.address)).is_alive())
        .collect();
    if chosen.is_empty() {
        return Err("nothing to restart".into());
    }
    let keys: Vec<CommandKey> = chosen.iter().map(|c| CommandKey::new(slug, &c.address)).collect();
    stop_keys(engine, &keys);
    for k in &keys {
        let _ = engine.start(k);
    }
    let out: Vec<Value> = chosen
        .iter()
        .map(|c| command_json(c, &engine.state_of(&CommandKey::new(slug, &c.address))))
        .collect();
    Ok(json!({ "commands": out }))
}

fn logs(engine: &Engine, slug: &str, address: &str, lines: usize) -> Result<Value, String> {
    commands_for(engine, slug, Some(address), true)?
        .into_iter()
        .find(|c| c.address == address)
        .ok_or_else(|| format!("{address:?} is a group; name one command"))?;
    let key = CommandKey::new(slug, address);
    if engine.session(&key).is_none() {
        return Err(format!("{slug}/{address} has not run since Switchboard started"));
    }
    Ok(json!({ "text": engine.log_text(&key, lines), "state": state_json(&engine.state_of(&key)) }))
}

// ----- status ----------------------------------------------------------------

fn status(engine: &Engine, only: Option<&str>) -> Result<Value, String> {
    let projects = engine.projects();
    if let Some(slug) = only {
        if !projects.iter().any(|p| p.slug == slug) {
            return Err(EngineError::UnknownProject(slug.to_string()).to_string());
        }
    }
    let out: Vec<Value> = projects
        .iter()
        .filter(|p| only.map_or(true, |s| s == p.slug))
        .map(|p| match &p.effective {
            Err(e) => json!({ "slug": p.slug, "error": e.to_string() }),
            Ok(cfg) => {
                let commands: Vec<Value> = cfg
                    .resolve_commands()
                    .iter()
                    .map(|c| command_json(c, &engine.state_of(&CommandKey::new(&p.slug, &c.address))))
                    .collect();
                json!({ "slug": p.slug, "name": cfg.name, "home": cfg.home, "commands": commands })
            }
        })
        .collect();
    Ok(json!({ "projects": out }))
}

fn command_json(c: &ResolvedCommand, state: &CommandState) -> Value {
    let mut v = json!({
        "address": c.address,
        "name": c.display_name,
        "group": c.group.as_ref().map(|g| g.id.clone()),
        "run": c.run,
        "cwd": c.cwd,
        "autostart": c.autostart,
        "terminal": c.terminal,
    });
    if let (Value::Object(m), Value::Object(s)) = (&mut v, state_json(state)) {
        m.extend(s);
    }
    v
}

fn secs(t: Option<SystemTime>) -> Option<f64> {
    t.and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d| d.as_secs_f64())
}

pub fn status_name(s: &Status) -> &'static str {
    match s {
        Status::Stopped => "stopped",
        Status::Starting => "starting",
        Status::Running => "running",
        Status::Exited => "exited",
        Status::Crashed => "crashed",
        Status::Error => "error",
    }
}

fn state_json(s: &CommandState) -> Value {
    json!({
        "status": status_name(&s.status),
        "pid": s.pid,
        "exit_code": s.exit_code,
        "signal": s.signal,
        "message": s.message,
        "port": s.port,
        "url": s.url,
        "needs_restart": s.needs_restart,
        // A crash restart is scheduled: the command is coming back, not finished.
        "restarting": s.restart_at.is_some(),
        "started_at": secs(s.started_at),
        "ended_at": secs(s.ended_at),
    })
}

// ----- client ----------------------------------------------------------------

/// A connection to a running app.
pub struct Client {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl Client {
    /// Connect, or `None` when no app is serving `path`.
    pub fn connect(path: &Path) -> Option<Client> {
        let stream = UnixStream::connect(path).ok()?;
        let writer = stream.try_clone().ok()?;
        Some(Client { reader: BufReader::new(stream), writer })
    }

    /// Send one request. `Err` carries the app's error message.
    pub fn call(&mut self, req: &Request) -> Result<Value, String> {
        let line = serde_json::to_string(req).map_err(|e| e.to_string())?;
        writeln!(self.writer, "{line}").map_err(|e| format!("lost the connection to Switchboard: {e}"))?;
        let mut reply = String::new();
        self.reader
            .read_line(&mut reply)
            .map_err(|e| format!("lost the connection to Switchboard: {e}"))?;
        if reply.is_empty() {
            return Err("Switchboard closed the connection".into());
        }
        let v: Value = serde_json::from_str(&reply).map_err(|e| format!("bad reply: {e}"))?;
        if v["ok"].as_bool() == Some(true) {
            Ok(v)
        } else {
            Err(v["error"].as_str().unwrap_or("unknown error").to_string())
        }
    }
}

/// Poll `path` until an app answers or `timeout` passes.
pub fn wait_for_server(path: &Path, timeout: Duration) -> Option<Client> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(mut c) = Client::connect(path) {
            if c.call(&Request::Ping).is_ok() {
                return Some(c);
            }
        }
        if std::time::Instant::now() > deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

// ----- attach ----------------------------------------------------------------

/// What the writer thread of an attached connection sends.
enum Outgoing {
    Bytes(Vec<u8>),
    /// A new run began: size its terminal to the viewer before its output goes out.
    Fit,
}

/// Stream one command's terminal over `out`, reading input and size frames from `reader`.
/// Survives restarts: output is matched by command, not by session.
fn attach(engine: Weak<Engine>, slug: &str, address: &str, size: (u16, u16), mut reader: BufReader<UnixStream>, mut out: UnixStream) {
    let Some(e) = engine.upgrade() else { return };
    if e.project(slug).is_none() {
        e.reload();
    }
    let cmd = commands_for(&e, slug, Some(address), true).and_then(|cmds| {
        cmds.into_iter().find(|c| c.address == address).ok_or_else(|| format!("{address:?} is a group; name one command"))
    });
    let cmd = match cmd {
        Ok(c) => c,
        Err(err) => {
            let _ = writeln!(out, "{}", fail(&err));
            return;
        }
    };
    let key = CommandKey::new(slug, address);
    let size = Arc::new(std::sync::Mutex::new(size));
    {
        let (c, r) = *size.lock().unwrap();
        e.resize(&key, c, r);
    }

    // Engine events arrive on its reader threads, so the listener only queues; a slow viewer
    // must never hold the engine up.
    let (tx, rx) = std::sync::mpsc::channel::<Outgoing>();
    let watched = key.clone();
    let listener = e.subscribe(Arc::new(move |ev: &crate::engine::Event| match ev {
        crate::engine::Event::Output { key, bytes } if *key == watched => {
            let _ = tx.send(Outgoing::Bytes(bytes.clone()));
        }
        crate::engine::Event::Status { key, state } if *key == watched => {
            if state.status == Status::Starting {
                let _ = tx.send(Outgoing::Fit);
                // The app's run separator, so one run reads apart from the last.
                let _ = tx.send(Outgoing::Bytes(note(&format!("started {}", local_clock())).into_bytes()));
            }
            if let Some(n) = attach_note(state) {
                let _ = tx.send(Outgoing::Bytes(n.into_bytes()));
            }
        }
        _ => {}
    }));

    let state = e.state_of(&key);
    let reply = json!({ "ok": true, "data": { "name": cmd.display_name, "state": state_json(&state) } });
    let mut opening = format!("{reply}\n").into_bytes();
    // What it printed so far, as the app's terminal views replay it.
    opening.extend(e.raw_tail(&key));
    if !state.is_alive() {
        opening.extend(attach_note(&state).unwrap_or_else(|| note("stopped · Enter starts it")).into_bytes());
    }
    drop(e);
    if out.write_all(&opening).is_err() {
        if let Some(e) = engine.upgrade() {
            e.unsubscribe(listener);
        }
        return;
    }

    let writer = {
        let (engine, key, size) = (engine.clone(), key.clone(), Arc::clone(&size));
        let Ok(mut w) = out.try_clone() else { return };
        std::thread::spawn(move || {
            let mut fit = false;
            for msg in rx {
                match msg {
                    Outgoing::Fit => fit = true,
                    Outgoing::Bytes(b) => {
                        // The new session exists once it has printed; size it then.
                        if fit {
                            if let Some(e) = engine.upgrade() {
                                let (c, r) = *size.lock().unwrap();
                                e.resize(&key, c, r);
                            }
                            fit = false;
                        }
                        if w.write_all(&b).is_err() {
                            break;
                        }
                    }
                }
            }
            // The viewer went away: make the frame reader below stop too.
            let _ = w.shutdown(std::net::Shutdown::Both);
        })
    };

    let mut header = [0u8; 5];
    while std::io::Read::read_exact(&mut reader, &mut header).is_ok() {
        let len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;
        if len > 1 << 20 {
            break;
        }
        let mut payload = vec![0u8; len];
        if std::io::Read::read_exact(&mut reader, &mut payload).is_err() {
            break;
        }
        let Some(e) = engine.upgrade() else { break };
        match header[0] {
            FRAME_INPUT => {
                // Not running: Enter starts it, as the note says.
                if e.write_input(&key, &payload).is_err() && payload.contains(&b'\r') {
                    let _ = e.start(&key);
                }
            }
            FRAME_RESIZE if len == 4 => {
                let (c, r) = (u16::from_be_bytes([payload[0], payload[1]]), u16::from_be_bytes([payload[2], payload[3]]));
                if c > 0 && r > 0 {
                    *size.lock().unwrap() = (c, r);
                    e.resize(&key, c, r);
                }
            }
            _ => {}
        }
    }
    if let Some(e) = engine.upgrade() {
        // Dropping the listener drops the queue's sender, which ends the writer.
        e.unsubscribe(listener);
    }
    let _ = out.shutdown(std::net::Shutdown::Both);
    let _ = writer.join();
}

/// A dim line between runs, so a viewer sees what happened without a status bar.
fn attach_note(state: &CommandState) -> Option<String> {
    let restart = state.restart_at.is_some();
    let text = match state.status {
        Status::Starting | Status::Running => return None,
        Status::Crashed if restart => state.message.clone().unwrap_or_else(|| "crashed, restarting".into()),
        Status::Crashed => match state.exit_code {
            Some(c) => format!("crashed with exit code {c} · Enter starts it again"),
            None => "crashed · Enter starts it again".into(),
        },
        Status::Exited => "finished · Enter runs it again".into(),
        Status::Stopped => "stopped · Enter starts it".into(),
        Status::Error => format!("{} · Enter tries again", state.message.clone().unwrap_or_else(|| "could not start".into())),
    };
    Some(note(&text))
}

/// HH:MM:SS in local time.
fn local_clock() -> String {
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&now, &mut tm) };
    format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
}

fn note(text: &str) -> String {
    format!("\r\n\x1b[0;2m── {text} ──\x1b[0m\r\n")
}
