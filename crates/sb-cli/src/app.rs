//! Commands that drive the running app through its control socket: what runs, runs in the
//! app's own terminals, where the author can see it.

use anyhow::{anyhow, bail, Result};
use crate::paint::Paint;
use sb_core::control::{self, Client, Request};
use serde_json::Value;
use std::time::{Duration, Instant};

const BUNDLE_ID: &str = "dev.waky.switchboard";

/// `kinpay`, `kinpay/backend` or `kinpay/backend/migrate`, plus an optional separate target.
pub fn split_target(arg: &str, target: Option<String>) -> (String, Option<String>) {
    match arg.split_once('/') {
        Some((p, rest)) if !rest.is_empty() => (p.to_string(), Some(rest.to_string())),
        _ => (arg.trim_end_matches('/').to_string(), target),
    }
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
    // A clean environment, so the app does not inherit the caller's (an agent session's
    // variables would leak into every command it runs).
    let mut open = std::process::Command::new("/usr/bin/open");
    open.env_clear();
    for var in ["HOME", "USER", "LOGNAME", "SWITCHBOARD_HOME"] {
        if let Ok(v) = std::env::var(var) {
            open.env(var, v);
        }
    }
    open.env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
    if !front {
        open.arg("-g");
    }
    let ok = open.args(["-b", BUNDLE_ID]).status().map(|s| s.success()).unwrap_or(false);
    if !ok {
        bail!("Switchboard.app is not installed (no app with bundle id {BUNDLE_ID})");
    }
    control::wait_for_server(&sock, Duration::from_secs(20))
        .map(Some)
        .ok_or_else(|| anyhow!("Switchboard did not answer at {} within 20 s", sock.display()))
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
