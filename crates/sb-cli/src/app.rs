//! Commands that drive the running app through its control socket: what runs, runs in the
//! app's own terminals, where the author can see it.

use anyhow::{anyhow, bail, Result};
use crate::paint::Paint;
use sb_core::control::{self, Client, Request};
use serde_json::Value;
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
const BUNDLE_ID: &str = "dev.waky.switchboard";
/// The systemd user unit that runs `switchboard serve`, when installed.
#[cfg(not(target_os = "macos"))]
pub const SERVICE: &str = "switchboard.service";

/// `kinpay`, `kinpay/backend` or `kinpay/backend/migrate`, plus an optional separate target.
pub fn split_target(arg: &str, target: Option<String>) -> (String, Option<String>) {
    match arg.split_once('/') {
        Some((p, rest)) if !rest.is_empty() => (p.to_string(), Some(rest.to_string())),
        _ => (arg.trim_end_matches('/').to_string(), target),
    }
}

/// A clean environment, so the host does not inherit the caller's (an agent session's
/// variables would leak into every command it runs).
fn clean_env(cmd: &mut std::process::Command) {
    cmd.env_clear();
    for var in ["HOME", "USER", "LOGNAME", "SWITCHBOARD_HOME"] {
        if let Ok(v) = std::env::var(var) {
            cmd.env(var, v);
        }
    }
    cmd.env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
}

/// Start whatever hosts the engine: the app on macOS.
#[cfg(target_os = "macos")]
fn launch_host(front: bool) -> Result<()> {
    let mut open = std::process::Command::new("/usr/bin/open");
    clean_env(&mut open);
    if !front {
        open.arg("-g");
    }
    let ok = open.args(["-b", BUNDLE_ID]).status().map(|s| s.success()).unwrap_or(false);
    if !ok {
        bail!("Switchboard.app is not installed (no app with bundle id {BUNDLE_ID})");
    }
    Ok(())
}

/// Start whatever hosts the engine: elsewhere there is no app, so `switchboard serve`, through
/// the systemd user unit when one is installed, else in the background with its output in
/// `serve.log` in the app folder.
#[cfg(not(target_os = "macos"))]
fn launch_host(_front: bool) -> Result<()> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    let systemd = Command::new("systemctl")
        .args(["--user", "cat", SERVICE])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if systemd {
        let ok = Command::new("systemctl").args(["--user", "start", SERVICE]).status().map(|s| s.success()).unwrap_or(false);
        if !ok {
            bail!("systemctl --user start {SERVICE} failed; see journalctl --user -u {SERVICE}");
        }
        return Ok(());
    }
    let exe = std::env::current_exe()?;
    let log_path = sb_core::project::app_support_dir().join("serve.log");
    if let Some(dir) = log_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let log = std::fs::OpenOptions::new().create(true).append(true).open(&log_path)?;
    let mut serve = Command::new(exe);
    clean_env(&mut serve);
    serve
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        // Its own process group, so the caller's Ctrl+C or exit does not take it down.
        .process_group(0)
        .spawn()?;
    Ok(())
}

/// Connect to the app. With `launch`, start it (in the background unless `front`) when it
/// is not running.
pub fn connect(launch: bool, front: bool) -> Result<Option<Client>> {
    let sock = control::socket_path();
    if let Some(mut c) = Client::connect(&sock) {
        if c.call(&Request::Ping).is_ok() {
            return Ok(Some(c));
        }
    }
    if !launch {
        return Ok(None);
    }
    eprintln!("{} starting Switchboard", "switchboard:".dimmed());
    launch_host(front)?;
    control::wait_for_server(&sock, Duration::from_secs(20))
        .map(Some)
        .ok_or_else(|| anyhow!("Switchboard did not answer at {} within 20 s", sock.display()))
}

/// A client for the running host, launching it when `launch` is set.
pub fn client(launch: bool) -> Result<Client> {
    need(connect(launch, false)?)
}

fn need(client: Option<Client>) -> Result<Client> {
    client.ok_or_else(|| anyhow!("Switchboard is not running. `switchboard start <project>` launches it."))
}

fn call(client: &mut Client, req: Request) -> Result<Value> {
    client.call(&req).map_err(|e| anyhow!(e))
}

// ----- status -----------------------------------------------------------------

pub fn status(project: Option<String>, json: bool, projects_dir: &std::path::Path) -> Result<i32> {
    let Some(mut client) = connect(false, false)? else {
        if json {
            println!("{}", serde_json::json!({ "app_running": false }));
            return Ok(0);
        }
        eprintln!("{} Switchboard is not running; showing the projects on disk", "switchboard:".dimmed());
        return crate::list(projects_dir);
    };
    let v = call(&mut client, Request::Status { project })?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v["projects"])?);
        return Ok(0);
    }
    for p in v["projects"].as_array().into_iter().flatten() {
        if let Some(e) = p["error"].as_str() {
            println!("{}  {}", p["slug"].as_str().unwrap_or("").bold(), "invalid config".red());
            for line in e.lines() {
                println!("    {}", line.red());
            }
            println!();
            continue;
        }
        println!("{}  {}", p["name"].as_str().unwrap_or("").bold(), format!("({})", p["slug"].as_str().unwrap_or("")).dimmed());
        for c in p["commands"].as_array().into_iter().flatten() {
            print_command(c);
        }
        println!();
    }
    Ok(0)
}

fn print_command(c: &Value) {
    let status = c["status"].as_str().unwrap_or("stopped");
    let dot = match status {
        "running" => "●".green().to_string(),
        "starting" => "●".yellow().to_string(),
        "crashed" | "error" => "●".red().to_string(),
        "exited" => "✓".dimmed().to_string(),
        _ => "○".dimmed().to_string(),
    };
    let mut detail = match (status, c["exit_code"].as_i64()) {
        ("exited" | "crashed", Some(code)) => format!("{status} ({code})"),
        _ => status.to_string(),
    };
    if let Some(url) = c["url"].as_str() {
        detail.push_str(&format!("  {url}"));
    }
    if let Some(m) = c["message"].as_str() {
        detail.push_str(&format!("  {m}"));
    }
    if c["needs_restart"].as_bool() == Some(true) {
        detail.push_str("  (restart to apply config changes)");
    }
    println!("    {dot} {:<28} {}", c["address"].as_str().unwrap_or(""), detail);
}

// ----- start / stop / restart ----------------------------------------------------

pub fn start(arg: &str, target: Option<String>) -> Result<i32> {
    let (project, target) = split_target(arg, target);
    let mut client = need(connect(true, false)?)?;
    let v = call(&mut client, Request::Start { project, target })?;
    report("started", &v);
    Ok(0)
}

pub fn stop(arg: &str, target: Option<String>) -> Result<i32> {
    let (project, target) = split_target(arg, target);
    let Some(mut client) = connect(false, false)? else {
        println!("Switchboard is not running; nothing to stop.");
        return Ok(0);
    };
    let v = call(&mut client, Request::Stop { project, target })?;
    if v["commands"].as_array().is_some_and(|a| a.is_empty()) {
        println!("Nothing was running.");
    }
    report("stopped", &v);
    Ok(0)
}

pub fn restart(arg: &str, target: Option<String>) -> Result<i32> {
    let (project, target) = split_target(arg, target);
    let mut client = need(connect(true, false)?)?;
    let v = call(&mut client, Request::Restart { project, target })?;
    report("restarted", &v);
    Ok(0)
}

fn report(verb: &str, v: &Value) {
    for c in v["commands"].as_array().into_iter().flatten() {
        let err = matches!(c["status"].as_str(), Some("error"));
        if err {
            println!("{} {}  {}", "failed".red(), c["address"].as_str().unwrap_or(""), c["message"].as_str().unwrap_or(""));
        } else {
            println!("{} {}", verb.green(), c["address"].as_str().unwrap_or(""));
        }
    }
}

// ----- logs / wait / open ----------------------------------------------------------

pub fn logs(arg: &str, target: Option<String>, tail: usize) -> Result<i32> {
    let (project, address) = split_target(arg, target);
    let address = address.ok_or_else(|| anyhow!("name a command, e.g. {project}/backend/api"))?;
    let mut client = need(connect(false, false)?)?;
    let v = call(&mut client, Request::Logs { project, address, lines: tail })?;
    println!("{}", v["text"].as_str().unwrap_or(""));
    Ok(0)
}

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum WaitFor {
    /// It printed the URL it listens on
    Ready,
    /// Its process is up
    Running,
    /// It finished; exits with the command's own exit code
    Exit,
}

pub fn wait(arg: &str, target: Option<String>, until: WaitFor, timeout: u64) -> Result<i32> {
    let (project, address) = split_target(arg, target);
    let address = address.ok_or_else(|| anyhow!("name a command, e.g. {project}/backend/api"))?;
    let mut client = need(connect(false, false)?)?;
    let deadline = Instant::now() + Duration::from_secs(timeout);
    let name = format!("{project}/{address}");
    loop {
        let v = call(&mut client, Request::Status { project: Some(project.clone()) })?;
        let c = v["projects"][0]["commands"]
            .as_array()
            .and_then(|a| a.iter().find(|c| c["address"] == address.as_str()))
            .cloned()
            .ok_or_else(|| anyhow!("{project} has no command {address:?}"))?;
        let status = c["status"].as_str().unwrap_or("stopped");
        let code = c["exit_code"].as_i64();
        match (until, status) {
            (_, "stopped") => bail!("{name} is not running; `switchboard start {name}` starts it"),
            (WaitFor::Ready, "running" | "starting") if c["url"].is_string() => {
                println!("{} {name}  {}", "ready".green(), c["url"].as_str().unwrap());
                return Ok(0);
            }
            (WaitFor::Running, "running") => {
                println!("{} {name}", "running".green());
                return Ok(0);
            }
            (WaitFor::Exit, "exited") => {
                println!("{} {name}", "done".green());
                return Ok(0);
            }
            // Crashed with a restart scheduled: waiting for it to be up means waiting through it.
            (WaitFor::Ready | WaitFor::Running, "crashed") if c["restarting"] == true => {}
            (WaitFor::Exit, "crashed") => {
                failed(&mut client, &project, &address, &c);
                return Ok(code.map(|c| c as i32).filter(|c| *c != 0).unwrap_or(1));
            }
            (_, "crashed" | "error" | "exited") => {
                failed(&mut client, &project, &address, &c);
                return Ok(1);
            }
            _ => {}
        }
        if Instant::now() > deadline {
            let hint = if until == WaitFor::Ready && status == "running" {
                " (it is running but has not printed a URL; `--for running` waits for the process only)"
            } else {
                ""
            };
            eprintln!("{} {name} is still {status} after {timeout} s{hint}", "timeout:".yellow().bold());
            print_tail(&mut client, &project, &address);
            return Ok(124);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn failed(client: &mut Client, project: &str, address: &str, c: &Value) {
    let status = c["status"].as_str().unwrap_or("");
    let mut why = status.to_string();
    if let Some(code) = c["exit_code"].as_i64() {
        why.push_str(&format!(" with exit code {code}"));
    }
    if let Some(m) = c["message"].as_str() {
        why.push_str(&format!(": {m}"));
    }
    eprintln!("{} {project}/{address} {why}", "failed:".red().bold());
    print_tail(client, project, address);
}

fn print_tail(client: &mut Client, project: &str, address: &str) {
    if let Ok(v) = client.call(&Request::Logs { project: project.into(), address: address.into(), lines: 40 }) {
        let text = v["text"].as_str().unwrap_or("");
        if !text.is_empty() {
            eprintln!("{}", "--- last 40 lines ---".dimmed());
            eprintln!("{text}");
        }
    }
}

/// `switchboard keep <project> [folder]`: a project's home, suggested or adopted.
pub fn keep(projects_dir: &std::path::Path, project: &str, folder: Option<String>, json: bool) -> Result<i32> {
    let adopting = folder.is_some();
    let v = if adopting {
        // Adopting a home changes the app's live config, so it goes through the running app.
        let mut client = need(connect(true, false)?)?;
        let folder = folder.map(|f| std::fs::canonicalize(&f).map(|p| p.display().to_string()).unwrap_or(f));
        call(&mut client, Request::Keep { project: project.to_string(), folder })?
    } else {
        // A suggestion only reads the project's config: no app needed.
        let dir = projects_dir.join(project);
        if !dir.join(sb_core::project::CONFIG_FILE).is_file() {
            anyhow::bail!("no project {project} in {}", projects_dir.display());
        }
        let p = sb_core::project::Project::load(&dir);
        serde_json::json!({ "home": p.home(), "suggestion": sb_core::home::suggest(&p) })
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(0);
    }
    let home = v["home"].as_str();
    if adopting {
        match (home, v["kept"].as_bool()) {
            (Some(h), Some(true)) => println!("kept {project}'s settings in {h}/.switchboard/project.yaml"),
            (Some(h), _) => println!("{project} already keeps its settings in {h}"),
            _ => {}
        }
        return Ok(0);
    }
    match home {
        Some(h) => println!("{project} keeps its settings in {}", h.bold()),
        None => println!("{project} has no home yet: its settings live only in Switchboard's app folder"),
    }
    let s = &v["suggestion"];
    match s["suggested"].as_str() {
        Some(p) => println!("suggested: {}  {}", p.bold(), format!("({})", s["reason"].as_str().unwrap_or("")).dimmed()),
        None => println!("{}", "no suggestion: pick one of these".dimmed()),
    }
    let others: Vec<&str> = s["candidates"].as_array().into_iter().flatten().filter_map(|c| c.as_str())
        .filter(|c| Some(*c) != s["suggested"].as_str()).collect();
    if !others.is_empty() {
        println!("other folders:");
        for c in others {
            println!("  {c}");
        }
    }
    if home.is_none() {
        let to = s["suggested"].as_str().unwrap_or("<folder>");
        println!("{}", format!("to keep it there: switchboard keep {project} {to}").dimmed());
    }
    Ok(0)
}

pub fn open(project: Option<String>) -> Result<i32> {
    let project = project.map(|p| split_target(&p, None).0);
    let mut client = need(connect(true, true)?)?;
    call(&mut client, Request::Open { project })?;
    Ok(0)
}

// ----- attach ----------------------------------------------------------------------

/// Ctrl+]: leave the command running and go back to the shell.
const DETACH: u8 = 0x1d;

struct RawMode;
impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

/// This terminal becomes the command's terminal: its output, your keystrokes, this window's
/// size. Ctrl+] detaches; the command keeps running under Switchboard.
pub fn attach(arg: &str, target: Option<String>) -> Result<i32> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    let (project, address) = split_target(arg, target);
    let address = address.ok_or_else(|| anyhow!("name a command, e.g. {project}/backend/api"))?;
    // Make sure something serves the socket (launching it if needed), then take a connection
    // of our own: once attached it carries raw bytes, not the client's JSON lines.
    need(connect(true, false)?)?;
    let mut stream = UnixStream::connect(control::socket_path())?;
    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let req = Request::Attach { project: project.clone(), address: address.clone(), cols, rows };
    writeln!(stream, "{}", serde_json::to_string(&req)?)?;

    // The reply line, read a byte at a time so none of the output after it is swallowed.
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if stream.read(&mut byte)? == 0 {
            bail!("Switchboard closed the connection");
        }
        if byte[0] == b'\n' {
            break;
        }
        line.push(byte[0]);
    }
    let reply: Value = serde_json::from_slice(&line)?;
    if reply["ok"] != true {
        bail!("{}", reply["error"].as_str().unwrap_or("attach failed"));
    }
    let name = reply["data"]["name"].as_str().unwrap_or(&address).to_string();

    let mut stdout = std::io::stdout();
    // The pane or window title, for Herdr's tab and pane labels.
    write!(stdout, "\x1b]2;{project} · {name}\x07")?;
    stdout.flush()?;
    let raw = crossterm::tty::IsTty::is_tty(&std::io::stdin())
        .then(|| crossterm::terminal::enable_raw_mode().ok().map(|_| RawMode))
        .flatten();

    let writer = Arc::new(Mutex::new(stream.try_clone()?));
    let detached = Arc::new(AtomicBool::new(false));
    {
        let (writer, detached, stream) = (Arc::clone(&writer), Arc::clone(&detached), stream.try_clone()?);
        std::thread::spawn(move || {
            let mut stdin = std::io::stdin();
            let mut buf = [0u8; 4096];
            while let Ok(n) = stdin.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let chunk = &buf[..n];
                let (send, leave) = match chunk.iter().position(|b| *b == DETACH) {
                    Some(i) => (&chunk[..i], true),
                    None => (chunk, false),
                };
                if !send.is_empty() && writer.lock().unwrap().write_all(&control::frame(control::FRAME_INPUT, send)).is_err() {
                    break;
                }
                if leave {
                    detached.store(true, Ordering::SeqCst);
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                    break;
                }
            }
        });
    }
    {
        let (writer, detached) = (Arc::clone(&writer), Arc::clone(&detached));
        std::thread::spawn(move || {
            let mut last = (cols, rows);
            while !detached.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(200));
                let now = crossterm::terminal::size().unwrap_or(last);
                if now != last {
                    last = now;
                    if writer.lock().unwrap().write_all(&control::resize_frame(now.0, now.1)).is_err() {
                        break;
                    }
                }
            }
        });
    }

    let mut buf = [0u8; 16384];
    loop {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if stdout.write_all(&buf[..n]).and_then(|_| stdout.flush()).is_err() {
                    break;
                }
            }
        }
    }
    drop(raw);
    if detached.load(Ordering::SeqCst) {
        println!("\r\n{} {project}/{address} keeps running · `switchboard attach {project}/{address}` comes back", "detached:".dimmed());
    } else {
        println!("\r\n{} Switchboard stopped serving", "detached:".dimmed());
    }
    Ok(0)
}
