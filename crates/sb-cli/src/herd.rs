//! `switchboard herd`: a project as a Herdr workspace. A tab per group, a pane per command
//! that starts with the project, each pane attached to the command Switchboard runs, so
//! closing Herdr only stops watching.

use crate::app;
use crate::paint::Paint;
use anyhow::{anyhow, bail, Result};
use sb_core::control::Request;
use serde_json::Value;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// One `herdr` call; returns its `result`.
fn herdr(args: &[&str]) -> Result<Value> {
    let out = Command::new("herdr").args(args).stderr(Stdio::piped()).output().map_err(|e| anyhow!("herdr: {e} (is Herdr installed?)"))?;
    // Some calls (`pane run`) print nothing when they work.
    if out.status.success() && out.stdout.iter().all(u8::is_ascii_whitespace) {
        return Ok(Value::Null);
    }
    let v: Value = serde_json::from_slice(&out.stdout)
        .map_err(|_| anyhow!("herdr {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()))?;
    if let Some(err) = v.get("error") {
        bail!("herdr {}: {}", args.join(" "), err["message"].as_str().unwrap_or("failed"));
    }
    Ok(v["result"].clone())
}

/// Herdr's server, opening it in a terminal the way Omarchy's Super+Ctrl+Enter does when
/// it is not running.
fn ensure_herdr() -> Result<Vec<Value>> {
    if let Ok(r) = herdr(&["workspace", "list"]) {
        return Ok(r["workspaces"].as_array().cloned().unwrap_or_default());
    }
    eprintln!("{} opening Herdr", "switchboard:".dimmed());
    Command::new("omarchy-launch-terminal-herdr")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| anyhow!("Herdr is not running; open it (Super+Ctrl+Enter, or `herdr` in a terminal) and run this again"))?;
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(300));
        if let Ok(r) = herdr(&["workspace", "list"]) {
            return Ok(r["workspaces"].as_array().cloned().unwrap_or_default());
        }
    }
    bail!("Herdr did not start within 15 s")
}

/// A tab's worth of commands.
struct Tab {
    label: String,
    cwd: Option<String>,
    addresses: Vec<String>,
}

/// Groups in sidebar order, each with the commands that start with the project. Groups with
/// none get no tab; `switchboard attach` reaches any command.
fn tabs(project: &Value) -> Vec<Tab> {
    let mut tabs: Vec<Tab> = Vec::new();
    for c in project["commands"].as_array().into_iter().flatten() {
        if c["autostart"] != true || c["terminal"] == true {
            continue;
        }
        let address = c["address"].as_str().unwrap_or_default().to_string();
        // "Backend / uvicorn" → "Backend"; ungrouped commands share one tab.
        let label = match c["group"].as_str() {
            Some(_) => c["name"].as_str().and_then(|n| n.split(" / ").next()).unwrap_or("commands").to_string(),
            None => "commands".to_string(),
        };
        match tabs.iter_mut().find(|t| t.label == label) {
            Some(t) => t.addresses.push(address),
            None => tabs.push(Tab { label, cwd: c["cwd"].as_str().map(String::from), addresses: vec![address] }),
        }
    }
    tabs
}

/// Split `root` into `n` panes in a grid, columns first: ceil(√n) columns, rows spread across
/// them, every share even (the rule Omarchy's `hsl` uses). Returns pane ids in reading order.
fn grid(root: &str, n: usize, cwd: &str) -> Result<Vec<String>> {
    let split = |pane: &str, dir: &str, share: f64| -> Result<String> {
        let r = herdr(&["pane", "split", pane, "--direction", dir, "--ratio", &format!("{share:.4}"), "--cwd", cwd, "--no-focus"])?;
        r["pane"]["pane_id"].as_str().map(String::from).ok_or_else(|| anyhow!("herdr pane split returned no pane"))
    };
    let mut cols = 1;
    while cols * cols < n {
        cols += 1;
    }
    // The ratio is the share the existing pane keeps, so splitting the last column off at
    // 1/(remaining) leaves every column the same width.
    let mut columns = vec![root.to_string()];
    for k in 1..cols {
        let last = columns.last().unwrap().clone();
        columns.push(split(&last, "right", 1.0 / (cols - k + 1) as f64)?);
    }
    let mut panes = vec![Vec::new(); cols];
    for (i, col) in columns.iter().enumerate() {
        let rows = n / cols + usize::from(i < n % cols);
        let mut last = col.clone();
        panes[i].push(last.clone());
        for j in 1..rows {
            last = split(&last, "down", 1.0 / (rows - j + 1) as f64)?;
            panes[i].push(last.clone());
        }
    }
    // Reading order: across the top row, then the next.
    let mut ordered = Vec::new();
    for row in 0.. {
        let mut any = false;
        for col in &panes {
            if let Some(p) = col.get(row) {
                ordered.push(p.clone());
                any = true;
            }
        }
        if !any {
            break;
        }
    }
    Ok(ordered)
}

pub fn herd(project: &str) -> Result<i32> {
    let mut client = app::client(true)?;
    let status = client.call(&Request::Status { project: Some(project.to_string()) }).map_err(|e| anyhow!(e))?;
    let p = status["projects"].as_array().and_then(|a| a.first()).cloned().ok_or_else(|| anyhow!("no project {project:?}"))?;
    let name = p["name"].as_str().unwrap_or(project).to_string();

    let workspaces = ensure_herdr()?;
    if let Some(w) = workspaces.iter().find(|w| w["label"] == name.as_str()) {
        let id = w["workspace_id"].as_str().unwrap_or_default();
        herdr(&["workspace", "focus", id])?;
        println!("{} {name} is already herded; focused it", "ok".green());
        return Ok(0);
    }

    let tabs = tabs(&p);
    if tabs.is_empty() {
        bail!("{name} has no commands that start with it; `switchboard attach {project}/<group>/<command>` opens one");
    }
    // Start first, so panes attach to running commands rather than to "press Enter".
    let started = client.call(&Request::Start { project: project.to_string(), target: None }).map_err(|e| anyhow!(e))?;
    let n = started["commands"].as_array().map(|a| a.len()).unwrap_or(0);

    let exe = std::env::current_exe()?.display().to_string();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
    let mut panes_made = 0;
    let mut workspace_id = String::new();
    for (i, tab) in tabs.iter().enumerate() {
        let cwd = tab.cwd.clone().unwrap_or_else(|| home.clone());
        let created = if i == 0 {
            let r = herdr(&["workspace", "create", "--label", &name, "--cwd", &cwd, "--focus"])?;
            workspace_id = r["workspace"]["workspace_id"].as_str().unwrap_or_default().to_string();
            r
        } else {
            herdr(&["tab", "create", "--workspace", &workspace_id, "--label", &tab.label, "--cwd", &cwd, "--no-focus"])?
        };
        let tab_id = created["tab"]["tab_id"].as_str().unwrap_or_default().to_string();
        let root = created["root_pane"]["pane_id"].as_str().ok_or_else(|| anyhow!("herdr created no pane"))?.to_string();
        if i == 0 {
            herdr(&["tab", "rename", &tab_id, &tab.label])?;
        }
        for (pane, address) in grid(&root, tab.addresses.len(), &cwd)?.iter().zip(&tab.addresses) {
            herdr(&["pane", "run", pane, &exe, "attach", &format!("{project}/{address}")])?;
            panes_made += 1;
        }
    }
    println!(
        "{} {name}: {} tab{}, {panes_made} pane{}, {n} command{} started",
        "herded".green(),
        tabs.len(),
        if tabs.len() == 1 { "" } else { "s" },
        if panes_made == 1 { "" } else { "s" },
        if n == 1 { "" } else { "s" },
    );
    Ok(0)
}
