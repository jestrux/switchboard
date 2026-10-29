//! One live run of a command: its PTY, process group, status and scrollback.

use crate::config::ResolvedCommand;
use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

/// Scrollback lines kept in the parsed grid.
pub const SCROLLBACK_LINES: usize = 10_000;
/// Raw bytes kept for replaying into a freshly attached terminal view.
const RAW_TAIL_MAX: usize = 1024 * 1024;
const RAW_TAIL_KEEP: usize = 768 * 1024;

/// Identifies a command: project slug plus address (`group/command` or `command`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommandKey {
    pub project: String,
    pub address: String,
}

impl CommandKey {
    pub fn new(project: impl Into<String>, address: impl Into<String>) -> Self {
        CommandKey { project: project.into(), address: address.into() }
    }
}

impl std::fmt::Display for CommandKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.project, self.address)
    }
}

/// How a process ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exit {
    /// Exit code 0: shown as done.
    Clean,
    /// Non-zero exit code.
    Code(i32),
    /// Killed by a signal.
    Signal(String),
}

impl Exit {
    pub fn code(&self) -> Option<i32> {
        match self {
            Exit::Clean => Some(0),
            Exit::Code(c) => Some(*c),
            Exit::Signal(_) => None,
        }
    }
    pub fn is_crash(&self) -> bool {
        !matches!(self, Exit::Clean)
    }
}

/// The spawn configuration a session was started with.
#[derive(Debug, Clone)]
pub struct SpawnSpec {
    pub run: String,
    pub cwd: Option<std::path::PathBuf>,
    pub env: std::collections::BTreeMap<String, String>,
    /// An interactive shell: stopped with SIGHUP, like closing a terminal window.
    pub terminal: bool,
}

impl SpawnSpec {
    pub fn of(cmd: &ResolvedCommand) -> Self {
        SpawnSpec { run: cmd.run.clone(), cwd: cmd.cwd.clone(), env: cmd.env.clone(), terminal: cmd.terminal }
    }
    pub fn matches(&self, cmd: &ResolvedCommand) -> bool {
        self.run == cmd.run && self.cwd == cmd.cwd && self.env == cmd.env
    }
}

#[derive(Debug)]
pub enum SpawnError {
    MissingFolder(std::path::PathBuf),
    Pty(String),
}

impl std::fmt::Display for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpawnError::MissingFolder(p) => {
                write!(f, "Folder not found: {}", crate::config::collapse_tilde(p))
            }
            SpawnError::Pty(m) => write!(f, "Could not start: {m}"),
        }
    }
}

struct Inner {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    parser: vt100::Parser,
    raw_tail: Vec<u8>,
    size: (u16, u16),
}

/// Called from the session's worker threads.
pub trait SessionObserver: Send + Sync {
    fn on_output(&self, key: &CommandKey, bytes: &[u8]);
    /// The process produced output or has been alive long enough to count as running.
    fn on_running(&self, key: &CommandKey);
    fn on_exit(&self, key: &CommandKey, exit: Exit);
    /// The program announced the address it is listening on.
    fn on_listening(&self, _key: &CommandKey, _listening: crate::ports::Listening) {}
}

pub struct Session {
    pub key: CommandKey,
    pub pid: u32,
    pub started_at: SystemTime,
    pub started_instant: Instant,
    pub spec: SpawnSpec,
    inner: Mutex<Inner>,
    exited: AtomicBool,
    /// Set once the observer has been told about the exit (status is final).
    exit_reported: AtomicBool,
    /// Set when the user asked for a stop, so a crash afterwards never restarts.
    stop_requested: AtomicBool,
    running_reported: AtomicBool,
    observer: Arc<dyn SessionObserver>,
}

/// The user's login shell: `$SHELL`, else the account's shell from the user database (a
/// systemd service or a cleaned environment has no `$SHELL`), else `/bin/zsh`, else `/bin/sh`.
pub fn login_shell() -> String {
    let usable = |s: &String| !s.is_empty() && std::path::Path::new(s).exists();
    std::env::var("SHELL")
        .ok()
        .filter(usable)
        .or_else(|| {
            nix::unistd::User::from_uid(nix::unistd::getuid())
                .ok()
                .flatten()
                .map(|u| u.shell.to_string_lossy().into_owned())
                .filter(usable)
        })
        .or_else(|| Some("/bin/zsh".to_string()).filter(usable))
        .unwrap_or_else(|| "/bin/sh".to_string())
}

impl Session {
    /// Spawn `$SHELL -lc <run>` in its own PTY and process group.
    pub fn spawn(
        key: CommandKey,
        cmd: &ResolvedCommand,
        size: (u16, u16),
        observer: Arc<dyn SessionObserver>,
    ) -> Result<Arc<Session>, SpawnError> {
        if let Some(cwd) = &cmd.cwd {
            if !cwd.is_dir() {
                return Err(SpawnError::MissingFolder(cwd.clone()));
            }
        }
        let (cols, rows) = size;
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| SpawnError::Pty(e.to_string()))?;

        let shell = login_shell();
        let mut builder = CommandBuilder::new(&shell);
        if cmd.terminal {
            // An interactive login shell: reads the user's rc files, shows a prompt.
            builder.arg("-l");
        } else {
            builder.args(["-lc", &cmd.run]);
        }
        if let Some(cwd) = &cmd.cwd {
            builder.cwd(cwd);
        } else if let Some(home) = dirs::home_dir() {
            builder.cwd(home);
        }
        builder.env("TERM", "xterm-256color");
        builder.env("COLORTERM", "truecolor");
        builder.env("SWITCHBOARD", "1");
        for (k, v) in &cmd.env {
            builder.env(k, v);
        }

        // portable-pty's child runs setsid() before exec, so pid == pgid.
        let mut child = pair
            .slave
            .spawn_command(builder)
            .map_err(|e| SpawnError::Pty(e.to_string()))?;
        drop(pair.slave);
        let pid = child.process_id().ok_or_else(|| SpawnError::Pty("no pid".into()))?;

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| SpawnError::Pty(e.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| SpawnError::Pty(e.to_string()))?;

        let session = Arc::new(Session {
            key: key.clone(),
            pid,
            started_at: SystemTime::now(),
            started_instant: Instant::now(),
            spec: SpawnSpec::of(cmd),
            inner: Mutex::new(Inner {
                master: pair.master,
                writer,
                parser: vt100::Parser::new(rows, cols, SCROLLBACK_LINES),
                raw_tail: Vec::new(),
                size: (cols, rows),
            }),
            exited: AtomicBool::new(false),
            exit_reported: AtomicBool::new(false),
            stop_requested: AtomicBool::new(false),
            running_reported: AtomicBool::new(false),
            observer,
        });

        // Reader thread: PTY bytes -> parser, raw tail, observer.
        let s = Arc::clone(&session);
        std::thread::Builder::new()
            .name(format!("sb-read {key}"))
            .spawn(move || {
                let mut buf = [0u8; 16 * 1024];
                let mut ports = crate::ports::Scanner::default();
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let chunk = &buf[..n];
                            if let Some(listening) = ports.feed(chunk) {
                                s.observer.on_listening(&s.key, listening);
                            }
                            {
                                let mut inner = s.inner.lock().unwrap();
                                inner.parser.process(chunk);
                                inner.raw_tail.extend_from_slice(chunk);
                                if inner.raw_tail.len() > RAW_TAIL_MAX {
                                    let cut = inner.raw_tail.len() - RAW_TAIL_KEEP;
                                    // Prefer cutting right after a newline so replay starts on a fresh line.
                                    let cut = inner.raw_tail[cut..]
                                        .iter()
                                        .position(|&b| b == b'\n')
                                        .map(|p| cut + p + 1)
                                        .unwrap_or(cut);
                                    inner.raw_tail.drain(..cut);
                                }
                            }
                            s.mark_running();
                            s.observer.on_output(&s.key, chunk);
                        }
                    }
                }
            })
            .expect("spawn reader thread");

        // Waiter thread: reap the child and report how it ended.
        let s = Arc::clone(&session);
        std::thread::Builder::new()
            .name(format!("sb-wait {key}"))
            .spawn(move || {
                let exit = match child.wait() {
                    Ok(status) => {
                        if status.success() {
                            Exit::Clean
                        } else if let Some(sig) = status.signal() {
                            Exit::Signal(sig.to_string())
                        } else {
                            Exit::Code(status.exit_code() as i32)
                        }
                    }
                    Err(e) => Exit::Signal(format!("wait failed: {e}")),
                };
                s.exited.store(true, Ordering::SeqCst);
                // Give the reader a moment to drain the last bytes before status flips.
                std::thread::sleep(Duration::from_millis(30));
                // Undo modes the program left switched on, so the next run (or the user's
                // view) starts from a normal screen. Only what is actually set is reset.
                let cleanup = s.mode_cleanup_bytes();
                if !cleanup.is_empty() {
                    {
                        let mut inner = s.inner.lock().unwrap();
                        inner.parser.process(&cleanup);
                        inner.raw_tail.extend_from_slice(&cleanup);
                    }
                    s.observer.on_output(&s.key, &cleanup);
                }
                s.observer.on_exit(&s.key, exit);
                s.exit_reported.store(true, Ordering::SeqCst);
            })
            .expect("spawn waiter thread");

        // Starting -> running after a short grace period even with no output yet.
        let s = Arc::clone(&session);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            if !s.has_exited() {
                s.mark_running();
            }
        });

        Ok(session)
    }

    /// Escape sequences that return the terminal to a plain state, based on the parsed
    /// screen: leave the alternate screen, stop mouse reporting and bracketed paste,
    /// show the cursor, reset attributes. Empty when nothing is switched on.
    pub fn mode_cleanup_bytes(&self) -> Vec<u8> {
        let inner = self.inner.lock().unwrap();
        let screen = inner.parser.screen();
        let mut out = Vec::new();
        if screen.alternate_screen() {
            out.extend_from_slice(b"\x1b[?1049l");
        }
        if screen.mouse_protocol_mode() != vt100::MouseProtocolMode::None {
            out.extend_from_slice(b"\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l");
        }
        if screen.bracketed_paste() {
            out.extend_from_slice(b"\x1b[?2004l");
        }
        if screen.hide_cursor() {
            out.extend_from_slice(b"\x1b[?25h");
        }
        if screen.application_cursor() {
            out.extend_from_slice(b"\x1b[?1l");
        }
        if screen.application_keypad() {
            out.extend_from_slice(b"\x1b>");
        }
        if !out.is_empty() {
            out.extend_from_slice(b"\x1b[0m");
        }
        out
    }

    fn mark_running(&self) {
        if !self.running_reported.swap(true, Ordering::SeqCst) {
            self.observer.on_running(&self.key);
        }
    }

    pub fn has_exited(&self) -> bool {
        self.exited.load(Ordering::SeqCst)
    }

    pub fn stop_requested(&self) -> bool {
        self.stop_requested.load(Ordering::SeqCst)
    }

    pub fn uptime(&self) -> Duration {
        self.started_instant.elapsed()
    }

    /// Write keystrokes or pasted text to the process.
    pub fn write_input(&self, bytes: &[u8]) -> std::io::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        inner.writer.write_all(bytes)?;
        inner.writer.flush()
    }

    /// Resize the PTY; the kernel delivers SIGWINCH to the foreground process.
    pub fn resize(&self, cols: u16, rows: u16) {
        if cols == 0 || rows == 0 {
            return;
        }
        let mut inner = self.inner.lock().unwrap();
        if inner.size == (cols, rows) {
            return;
        }
        inner.size = (cols, rows);
        let _ = inner
            .master
            .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
        inner.parser.screen_mut().set_size(rows, cols);
    }

    pub fn size(&self) -> (u16, u16) {
        self.inner.lock().unwrap().size
    }

    /// Everything received so far (bounded), for replaying into a new terminal view.
    pub fn raw_tail(&self) -> Vec<u8> {
        self.inner.lock().unwrap().raw_tail.clone()
    }

    /// Plain text of the visible screen.
    pub fn screen_text(&self) -> String {
        self.inner.lock().unwrap().parser.screen().contents()
    }

    /// The last `lines` lines of output as plain text: the recorded output replayed through
    /// a fresh terminal, so colours, cursor moves and progress bars come out as they looked.
    pub fn log_text(&self, lines: usize) -> String {
        let (raw, (cols, rows)) = {
            let inner = self.inner.lock().unwrap();
            (inner.raw_tail.clone(), inner.size)
        };
        let mut parser = vt100::Parser::new(rows, cols, SCROLLBACK_LINES);
        parser.process(&raw);
        let screen = parser.screen_mut();
        screen.set_scrollback(usize::MAX);
        let back = screen.scrollback();
        let mut out: Vec<String> = Vec::new();
        // At scrollback offset k the top visible row is the k-th line from the end of the
        // scrollback, so stepping k down to 1 reads the scrollback in order.
        for k in (1..=back).rev() {
            screen.set_scrollback(k);
            out.extend(screen.rows(0, cols).next());
        }
        screen.set_scrollback(0);
        out.extend(screen.rows(0, cols));
        while out.last().is_some_and(|l| l.trim().is_empty()) {
            out.pop();
        }
        let start = out.len().saturating_sub(lines);
        out[start..].iter().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n")
    }

    /// Send a signal to the whole process group.
    pub fn signal(&self, sig: Signal) -> nix::Result<()> {
        killpg(Pid::from_raw(self.pid as i32), sig)
    }

    /// SIGINT, then SIGTERM after 3 s, then SIGKILL after a further 3 s.
    /// Blocks until the process has exited and its final status has been reported.
    pub fn stop_blocking(&self) {
        self.stop_requested.store(true, Ordering::SeqCst);
        if !self.has_exited() {
            // Interactive shells ignore SIGINT and SIGTERM; a hangup is what ends them.
            if self.spec.terminal {
                let _ = self.signal(Signal::SIGHUP);
                self.wait_exited(Duration::from_millis(1500));
            }
            let steps = [
                (Signal::SIGINT, Duration::from_secs(3)),
                (Signal::SIGTERM, Duration::from_secs(3)),
                (Signal::SIGKILL, Duration::from_secs(5)),
            ];
            for (sig, wait) in steps {
                if self.has_exited() {
                    break;
                }
                if self.signal(sig).is_err() && self.has_exited() {
                    break;
                }
                if self.wait_exited(wait) {
                    break;
                }
            }
        }
        self.wait_reported(Duration::from_secs(2));
    }

    /// Poll until the exit has been reported to the observer or `timeout` passes.
    pub fn wait_reported(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if self.exit_reported.load(Ordering::SeqCst) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        self.exit_reported.load(Ordering::SeqCst)
    }

    /// Mark that the user asked for a stop (suppresses crash restarts) without signalling.
    pub fn request_stop(&self) {
        self.stop_requested.store(true, Ordering::SeqCst);
    }

    /// Poll until the process has exited or `timeout` passes.
    pub fn wait_exited(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if self.has_exited() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        self.has_exited()
    }
}

/// Is a process (or group leader) with this pid still alive?
pub fn pid_alive(pid: u32) -> bool {
    nix::sys::signal::kill(Pid::from_raw(pid as i32), None).is_ok()
}

/// Does any process in this group still exist?
pub fn pgid_alive(pgid: u32) -> bool {
    killpg(Pid::from_raw(pgid as i32), None).is_ok()
}
