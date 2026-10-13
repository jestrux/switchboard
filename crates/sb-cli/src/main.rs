//! `sb`: the Switchboard command-line front end.

use anyhow::{anyhow, bail, Result};
use clap::{Parser, Subcommand};
use owo_colors::{OwoColorize, Style};
use sb_core::engine::{Engine, Event, Status};
use sb_core::project::{self, Conflict};
use sb_core::session::CommandKey;
use sb_core::{pidfile, ConfigError, ProjectConfig};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Parser)]
#[command(name = "sb", version, about = "Start, watch and stop a project's dev commands")]
struct Cli {
    /// Projects directory (default: ~/Library/Application Support/Switchboard/Projects)
    #[arg(long, global = true, env = "SB_PROJECTS_DIR")]
    projects_dir: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show projects, their commands and status
    List,
    /// Check a project folder (or config.yaml) and report errors
    Validate { path: PathBuf },
    /// Run a project's autostart commands, one group, or one command in the foreground
    Run {
        /// Project slug, or a path to a folder holding config.yaml
        project: String,
        /// `group` or `group/command` (ungrouped: `command`)
        target: Option<String>,
    },
    /// Validate and copy a project folder into the Projects directory
    Install {
        path: PathBuf,
        /// Replace an existing project with the same name
        #[arg(long)]
        replace: bool,
        /// Keep both, installing as name-2, name-3, …
        #[arg(long, conflicts_with = "replace")]
        keep_both: bool,
    },
    /// Kill sessions left behind by a crashed app or CLI
    Cleanup,
}

fn main() {
    let cli = Cli::parse();
    let projects_dir = cli.projects_dir.unwrap_or_else(project::default_projects_dir);
    let code = match cli.cmd {
        Cmd::List => list(&projects_dir),
        Cmd::Validate { path } => validate(&path),
        Cmd::Run { project, target } => run(&projects_dir, &project, target.as_deref()),
        Cmd::Install { path, replace, keep_both } => install(&projects_dir, &path, replace, keep_both),
        Cmd::Cleanup => cleanup(&projects_dir),
    };
    match code {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("{} {e:#}", "error:".red().bold());
            std::process::exit(2);
        }
    }
}

// ----- list -------------------------------------------------------------------

fn list(projects_dir: &Path) -> Result<i32> {
    let projects = project::load_projects(projects_dir);
    if projects.is_empty() {
        println!("No projects in {}", projects_dir.display());
        return Ok(0);
    }
    let pidfile_path = project::app_support_dir().join(pidfile::FILE_NAME);
    let live: HashMap<(String, String), u32> = pidfile::alive(&pidfile::read(&pidfile_path))
        .into_iter()
        .map(|e| ((e.project, e.address), e.pid))
        .collect();

    for p in &projects {
        match &p.config {
            Err(e) => {
                println!("{}  {}", p.slug.bold(), "invalid config".red());
                for line in e.messages() {
                    println!("    {}", line.red());
                }
            }
            Ok(cfg) => {
                println!("{}  {}", cfg.name.bold(), format!("({})", p.slug).dimmed());
                let mut last_group: Option<String> = None;
                for c in cfg.resolve_commands() {
                    let g = c.group.as_ref().map(|g| g.name.clone());
                    if g != last_group {
                        if let Some(name) = &g {
                            println!("  {}", name.underline());
                        }
                        last_group = g;
                    }
                    let (dot, status) = match live.get(&(p.slug.clone(), c.address.clone())) {
                        Some(pid) => ("●".green().to_string(), format!("running pid {pid}")),
                        None => ("○".dimmed().to_string(), "stopped".dimmed().to_string()),
                    };
                    let folder = c.cwd_raw.clone().unwrap_or_else(|| "~".to_string());
                    println!(
                        "    {dot} {:<24} {:<32} {}  {}",
                        c.address,
                        c.name,
                        folder.dimmed(),
                        status
                    );
                }
            }
        }
        println!();
    }
    Ok(0)
}

// ----- validate ---------------------------------------------------------------

fn validate(path: &Path) -> Result<i32> {
    let file = if path.is_dir() { path.join(project::CONFIG_FILE) } else { path.to_path_buf() };
    match ProjectConfig::load(&file) {
        Ok(cfg) => {
            let cmds = cfg.resolve_commands();
            println!(
                "{} {}: {} ({} group{}, {} command{})",
                "ok".green().bold(),
                file.display(),
                cfg.name,
                cfg.groups.len(),
                if cfg.groups.len() == 1 { "" } else { "s" },
                cmds.len(),
                if cmds.len() == 1 { "" } else { "s" },
            );
            let mut missing = 0;
            for c in &cmds {
                if let Some(cwd) = &c.cwd {
                    if !cwd.is_dir() {
                        missing += 1;
                        println!(
                            "  {} {}: folder not found: {}",
                            "warning".yellow(),
                            c.address,
                            c.cwd_raw.clone().unwrap_or_default()
                        );
                    }
                }
            }
            if missing > 0 {
                println!("  ({missing} command folder(s) missing on this machine; the config itself is valid)");
            }
            Ok(0)
        }
        Err(ConfigError::Invalid(errors)) => {
            println!("{} {}:", "invalid".red().bold(), file.display());
            for e in errors {
                println!("  - {e}");
            }
            Ok(1)
        }
        Err(e) => {
            println!("{} {}:\n  {e}", "invalid".red().bold(), file.display());
            Ok(1)
        }
    }
}

// ----- install / cleanup --------------------------------------------------------

fn install(projects_dir: &Path, path: &Path, replace: bool, keep_both: bool) -> Result<i32> {
    let conflict = if replace {
        Conflict::Replace
    } else if keep_both {
        Conflict::KeepBoth
    } else {
        Conflict::Fail
    };
    match project::install_project(path, projects_dir, conflict) {
        Ok(p) => {
            println!("{} installed {} as {}", "ok".green().bold(), p.name(), p.dir.display());
            Ok(0)
        }
        Err(project::InstallError::Exists(slug)) => {
            eprintln!(
                "a project folder named {slug:?} already exists; pass --replace or --keep-both"
            );
            Ok(1)
        }
        Err(e) => {
            eprintln!("{} {e}", "not installed:".red().bold());
            Ok(1)
        }
    }
}

fn cleanup(projects_dir: &Path) -> Result<i32> {
    let engine = Engine::new(projects_dir);
    let leftovers = engine.leftover_sessions();
    if leftovers.is_empty() {
        println!("no leftover sessions");
        return Ok(0);
    }
    for e in &leftovers {
        println!("killing {}:{} (pid {})", e.project, e.address, e.pid);
    }
    engine.kill_leftovers();
    Ok(0)
}

// ----- run ------------------------------------------------------------------------

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}
#[allow(non_camel_case_types)]
type libc_int = i32;

fn install_signal_handlers() {
    use nix_shim::*;
    unsafe {
        for sig in [SIGINT, SIGTERM, SIGHUP] {
            signal(sig, on_signal as *const () as usize);
        }
    }
}

/// Just enough libc to install plain signal handlers without another dependency.
mod nix_shim {
    pub const SIGINT: i32 = 2;
    pub const SIGHUP: i32 = 1;
    pub const SIGTERM: i32 = 15;
    extern "C" {
        pub fn signal(signum: i32, handler: usize) -> usize;
    }
}

const PALETTE: [fn(Style) -> Style; 6] = [
    |s| s.cyan(),
    |s| s.magenta(),
    |s| s.yellow(),
    |s| s.green(),
    |s| s.blue(),
    |s| s.bright_red(),
];

fn run(projects_dir: &Path, project: &str, target: Option<&str>) -> Result<i32> {
    // `sb run path/to/folder` runs a project folder directly.
    let as_path = Path::new(project);
    let (engine, slug) = if as_path.join(project::CONFIG_FILE).is_file() {
        let dir = as_path.canonicalize()?;
        let parent = dir.parent().ok_or_else(|| anyhow!("bad path"))?.to_path_buf();
        let slug = dir.file_name().unwrap().to_string_lossy().into_owned();
        (Engine::new(parent), slug)
    } else {
        (Engine::new(projects_dir), project.to_string())
    };

    let proj = engine
        .project(&slug)
        .ok_or_else(|| anyhow!("no project {slug:?} in {}", engine.projects_dir().display()))?;
    let cfg = match &proj.config {
        Ok(c) => c.clone(),
        Err(e) => bail!("project {slug:?} has an invalid config:\n{e}"),
    };

    // Which commands to run.
    let commands: Vec<sb_core::ResolvedCommand> = match target {
        None => cfg.resolve_commands().into_iter().filter(|c| c.autostart).collect(),
        Some(t) => {
            if let Some(c) = cfg.resolve(t) {
                vec![c]
            } else if cfg.group(t).is_some() {
                cfg.resolve_group(t).into_iter().filter(|c| c.autostart).collect()
            } else {
                bail!("{slug} has no command or group named {t:?}");
            }
        }
    };
    if commands.is_empty() {
        bail!("nothing to run (no autostart commands matched)");
    }
    let single = commands.len() == 1;

    // Output plumbing.
    let width = commands.iter().map(|c| c.display_name.len()).max().unwrap_or(0);
    let styles: HashMap<String, Style> = commands
        .iter()
        .enumerate()
        .map(|(i, c)| (c.address.clone(), PALETTE[i % PALETTE.len()](Style::new())))
        .collect();
    let printer = Arc::new(Mutex::new(Printer {
        at_line_start: commands.iter().map(|c| (c.address.clone(), true)).collect(),
        pending: HashMap::new(),
        names: commands.iter().map(|c| (c.address.clone(), c.display_name.clone())).collect(),
        styles,
        width,
        single,
    }));
    let p = printer.clone();
    engine.subscribe(Arc::new(move |e: &Event| {
        let mut p = p.lock().unwrap();
        match e {
            Event::Output { key, bytes } => p.output(&key.address, bytes),
            Event::Status { key, state } => p.status(&key.address, state),
            Event::ProjectsChanged => {}
        }
    }));

    // Terminal size: a single command gets the whole terminal; several share it minus the prefix.
    let (cols, rows) = crossterm::terminal::size().unwrap_or((120, 40));
    let prefix_width = if single { 0 } else { width as u16 + 3 };
    engine.set_default_size(cols.saturating_sub(prefix_width).max(20), rows.max(5));

    install_signal_handlers();

    let keys: Vec<CommandKey> = commands.iter().map(|c| CommandKey::new(&slug, &c.address)).collect();
    for key in &keys {
        if let Err(e) = engine.start(key) {
            eprintln!("{} {}: {e}", "failed".red().bold(), key.address);
        }
    }

    // A single command gets raw stdin so prompts and interactive programs work.
    let _raw = if single && crossterm::tty::IsTty::is_tty(&std::io::stdin()) {
        crossterm::terminal::enable_raw_mode().ok().map(|_| RawGuard)
    } else {
        None
    };
    if single {
        let engine = Arc::clone(&engine);
        let key = keys[0].clone();
        std::thread::spawn(move || {
            let mut stdin = std::io::stdin();
            let mut buf = [0u8; 1024];
            loop {
                match stdin.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if engine.write_input(&key, &buf[..n]).is_err() {
                            break;
                        }
                    }
                }
            }
        });
    }

    // Main loop: wait for everything to finish or for Ctrl+C; forward terminal resizes.
    let mut last_size = (cols, rows);
    let exit_code: i32 = loop {
        if INTERRUPTED.load(Ordering::SeqCst) {
            {
                let p = printer.lock().unwrap();
                p.note("stopping…");
            }
            engine.stop_everything_blocking();
            break 0;
        }
        let states: Vec<_> = keys.iter().map(|k| engine.state_of(k)).collect();
        if states.iter().all(|s| !s.is_alive()) {
            let worst = states
                .iter()
                .map(|s| match s.status {
                    Status::Exited | Status::Stopped => 0,
                    Status::Crashed => s.exit_code.unwrap_or(1).max(1),
                    Status::Error => 1,
                    _ => 0,
                })
                .max()
                .unwrap_or(0);
            // Allow a pending crash restart to proceed instead of exiting.
            if states.iter().any(|s| s.message.as_deref().map(|m| m.contains("restarting")).unwrap_or(false)) {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            break worst;
        }
        if let Ok(size) = crossterm::terminal::size() {
            if size != last_size {
                last_size = size;
                let (c, r) = size;
                for k in &keys {
                    engine.resize(k, c.saturating_sub(prefix_width).max(20), r.max(5));
                }
            }
        }
        printer.lock().unwrap().flush_stale();
        std::thread::sleep(Duration::from_millis(50));
    };
    printer.lock().unwrap().flush_all();
    Ok(exit_code)
}

struct RawGuard;
impl Drop for RawGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

struct Printer {
    at_line_start: HashMap<String, bool>,
    /// Partial line not yet printed, per command, with when it was last appended.
    pending: HashMap<String, (Vec<u8>, std::time::Instant)>,
    names: HashMap<String, String>,
    styles: HashMap<String, Style>,
    width: usize,
    single: bool,
}

/// How long a partial line may wait for its newline before being printed as is.
const PARTIAL_LINE_FLUSH: Duration = Duration::from_millis(150);

impl Printer {
    fn prefix(&self, address: &str) -> String {
        if self.single {
            return String::new();
        }
        let name = self.names.get(address).cloned().unwrap_or_default();
        let style = self.styles.get(address).copied().unwrap_or_default();
        format!("{} {} ", format!("{name:>w$}", w = self.width).style(style), "│".dimmed())
    }

    fn output(&mut self, address: &str, bytes: &[u8]) {
        if self.single {
            let mut out = std::io::stdout().lock();
            let _ = out.write_all(bytes);
            let _ = out.flush();
            return;
        }
        // Only complete lines go out immediately, so concurrent commands do not
        // interleave inside a line. The remainder waits briefly for its newline.
        let entry = self.pending.entry(address.to_string()).or_insert_with(|| (Vec::new(), std::time::Instant::now()));
        entry.0.extend_from_slice(bytes);
        entry.1 = std::time::Instant::now();
        let buf = std::mem::take(&mut entry.0);
        let cut = buf.iter().rposition(|&b| b == b'\n').map(|i| i + 1);
        let (complete, rest) = match cut {
            Some(i) => buf.split_at(i),
            None => (&buf[..0], &buf[..]),
        };
        if !complete.is_empty() {
            self.write_prefixed(address, complete);
        }
        if let Some(e) = self.pending.get_mut(address) {
            e.0 = rest.to_vec();
        }
    }

    /// Print partial lines that have waited long enough (called from a timer).
    fn flush_stale(&mut self) {
        let now = std::time::Instant::now();
        let stale: Vec<String> = self
            .pending
            .iter()
            .filter(|(_, (b, t))| !b.is_empty() && now.duration_since(*t) >= PARTIAL_LINE_FLUSH)
            .map(|(k, _)| k.clone())
            .collect();
        for address in stale {
            let bytes = std::mem::take(&mut self.pending.get_mut(&address).unwrap().0);
            self.write_prefixed(&address, &bytes);
        }
    }

    fn flush_all(&mut self) {
        let keys: Vec<String> = self.pending.keys().cloned().collect();
        for address in keys {
            let bytes = std::mem::take(&mut self.pending.get_mut(&address).unwrap().0);
            if !bytes.is_empty() {
                self.write_prefixed(&address, &bytes);
            }
        }
    }

    fn write_prefixed(&mut self, address: &str, bytes: &[u8]) {
        let prefix = self.prefix(address);
        let mut at_start = *self.at_line_start.get(address).unwrap_or(&true);
        let mut buf = Vec::with_capacity(bytes.len() + 64);
        for &b in bytes {
            if at_start {
                buf.extend_from_slice(prefix.as_bytes());
                at_start = false;
            }
            buf.push(b);
            if b == b'\n' {
                at_start = true;
            }
        }
        self.at_line_start.insert(address.to_string(), at_start);
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(&buf);
        let _ = out.flush();
    }

    fn status(&mut self, address: &str, state: &sb_core::CommandState) {
        let text = match state.status {
            Status::Starting => {
                if state.pid.is_some() {
                    return;
                }
                "starting".yellow().to_string()
            }
            Status::Running => format!("running (pid {})", state.pid.unwrap_or(0)).green().to_string(),
            Status::Exited => "done".green().to_string(),
            Status::Crashed => {
                let base = match (&state.exit_code, &state.signal) {
                    (_, Some(sig)) => format!("killed by {sig}"),
                    (Some(c), _) => format!("exited with code {c}"),
                    _ => "crashed".to_string(),
                };
                match &state.message {
                    Some(m) => format!("{base}; {m}").red().to_string(),
                    None => base.red().to_string(),
                }
            }
            Status::Error => state.message.clone().unwrap_or_else(|| "error".into()).red().to_string(),
            Status::Stopped => {
                if state.pid.is_none() {
                    return;
                }
                "stopped".dimmed().to_string()
            }
        };
        self.line(address, &text);
    }

    fn line(&mut self, address: &str, text: &str) {
        if let Some(e) = self.pending.get_mut(address) {
            let bytes = std::mem::take(&mut e.0);
            if !bytes.is_empty() {
                self.write_prefixed(address, &bytes);
            }
        }
        let mut out = std::io::stdout().lock();
        let at_start = *self.at_line_start.get(address).unwrap_or(&true);
        let mut s = String::new();
        if !at_start {
            s.push_str("\r\n");
        }
        if self.single {
            s.push_str(&format!("{} {}\r\n", "sb:".dimmed(), text));
        } else {
            s.push_str(&format!("{}{}\n", self.prefix(address), text));
        }
        self.at_line_start.insert(address.to_string(), true);
        let _ = out.write_all(s.as_bytes());
        let _ = out.flush();
    }

    fn note(&self, text: &str) {
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(format!("\r\n{} {}\r\n", "sb:".dimmed(), text).as_bytes());
        let _ = out.flush();
    }
}
