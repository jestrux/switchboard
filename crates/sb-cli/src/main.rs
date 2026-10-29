//! `switchboard`: the Switchboard command-line front end.

mod app;
mod board;
mod herd;
mod ui;
mod paint;

use anyhow::{anyhow, bail, Result};
use clap::{Parser, Subcommand};
use owo_colors::Style;
use paint::Paint;
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
#[command(name = "switchboard", version, about = "Start, watch and stop a project's dev commands")]
struct Cli {
    /// Projects directory (default: ~/Library/Application Support/Switchboard/Projects)
    #[arg(long, global = true, env = "SB_PROJECTS_DIR")]
    projects_dir: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// What runs in the app: every project's commands with status, port and URL
    Status {
        /// Only this project
        project: Option<String>,
        /// Machine-readable output
        #[arg(long)]
        json: bool,
    },
    /// Start commands in the app (launching it if needed): autostart ones, a group's, or one
    Start {
        /// `project`, `project/group` or `project/group/command`
        project: String,
        target: Option<String>,
    },
    /// Stop commands in the app and wait until they have exited
    Stop {
        /// `project`, `project/group` or `project/group/command`
        project: String,
        target: Option<String>,
    },
    /// Stop, then start again
    Restart {
        /// `project`, `project/group` or `project/group/command`
        project: String,
        target: Option<String>,
    },
    /// Print a command's recent output from the app, as plain text
    Logs {
        /// `project/group/command`
        command: String,
        target: Option<String>,
        /// How many lines
        #[arg(long, short = 'n', default_value_t = 100)]
        tail: usize,
    },
    /// Block until a command is ready, running or finished; on failure print its output
    Wait {
        /// `project/group/command`
        command: String,
        target: Option<String>,
        #[arg(long = "for", value_enum, default_value = "ready")]
        until: app::WaitFor,
        /// Seconds before giving up (exit code 124)
        #[arg(long, default_value_t = 120)]
        timeout: u64,
    },
    /// Become a command's terminal: its live output, your keystrokes, this window's size.
    /// Ctrl+] detaches and leaves it running. Enter starts it when it is not running
    Attach {
        /// `project/group/command`, or the project with the command as a second argument
        project: String,
        target: Option<String>,
    },
    /// The Mac app's window in this terminal: projects, their groups and commands with status,
    /// and the selected command's live terminal. Quitting stops nothing
    Ui,
    /// Open a project as a Herdr workspace: a tab per group, a pane per command that starts
    /// with it, each attached to the command Switchboard runs. Run again to focus it
    Herd { project: String },
    /// Bring the app forward, showing a project
    Open { project: Option<String> },
    /// Write the project's board (plans/06-board.md): cards, asks, the plan's steps, answers
    Board {
        /// The board's folder (default: this folder's project's board; or BOARD_DIR)
        #[arg(long)]
        dir: Option<PathBuf>,
        #[command(subcommand)]
        cmd: board::BoardCmd,
    },
    /// A project's home folder: with no folder, show it or suggest one; with a folder, keep
    /// the project's settings (and its board) in that repo. Only on the author's say-so.
    Keep {
        project: String,
        /// The repo folder to keep the settings in
        folder: Option<String>,
        /// Print JSON
        #[arg(long)]
        json: bool,
    },
    /// Change an existing command where it's defined: the project's config, or its group's repo
    /// commands.yaml. Only the fields you pass change; the file is rewritten, so its comments go
    EditCommand {
        /// `project/group/command`, or `project/command` when ungrouped
        target: String,
        /// What to run, through your login shell
        #[arg(long)]
        run: Option<String>,
        /// Folder to run in ("" for the group's)
        #[arg(long)]
        cwd: Option<String>,
        /// Display name ("" for the id)
        #[arg(long)]
        name: Option<String>,
        /// Set environment, KEY=VALUE (repeatable)
        #[arg(long = "env", value_name = "KEY=VALUE")]
        env: Vec<String>,
        /// Remove an environment variable (repeatable)
        #[arg(long = "unset-env", value_name = "KEY")]
        unset_env: Vec<String>,
        /// Start it with the project
        #[arg(long, conflicts_with = "no_autostart")]
        autostart: bool,
        /// Do not start it with the project
        #[arg(long)]
        no_autostart: bool,
        /// Restart it with backoff when it crashes
        #[arg(long, conflicts_with = "no_restart_on_crash")]
        restart_on_crash: bool,
        /// Do not restart it when it crashes
        #[arg(long)]
        no_restart_on_crash: bool,
    },
    /// Add a command to a project's config (or its group's repo commands.yaml)
    AddCommand {
        /// `project` or `project/group`
        project: String,
        /// Command id, unique in its group (becomes the address `group/id`)
        #[arg(long)]
        id: String,
        /// What to run, through your login shell
        #[arg(long)]
        run: String,
        /// Display name (default: the id)
        #[arg(long)]
        name: Option<String>,
        /// Folder to run in (default: the group's)
        #[arg(long)]
        cwd: Option<String>,
        /// Do not start it with the project
        #[arg(long)]
        no_autostart: bool,
        /// Restart it with backoff when it crashes
        #[arg(long)]
        restart_on_crash: bool,
        /// Environment, KEY=VALUE (repeatable)
        #[arg(long = "env", value_name = "KEY=VALUE")]
        env: Vec<String>,
        /// Creating a new group: its display name
        #[arg(long)]
        group_name: Option<String>,
        /// Creating a new group: the folder its commands run in
        #[arg(long)]
        group_cwd: Option<String>,
    },
    /// Show projects on disk and what is running, without the app
    List,
    /// Check a project folder (or config.yaml) and report errors
    Validate { path: PathBuf },
    /// Run commands here in the terminal instead of in the app (Ctrl+C stops them)
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
    /// Set a project up from a repo folder's .switchboard/project.yaml
    Import {
        /// Any member folder of the project (the newest project.yaml found wins)
        path: PathBuf,
        #[arg(long)]
        replace: bool,
        #[arg(long, conflicts_with = "replace")]
        keep_both: bool,
    },
    /// Convert Solo's solo.yml into a Switchboard project and install it
    ImportSolo {
        /// Folders containing solo.yml (or whose subfolders do). Several are merged into one project.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Print the converted config instead of installing it
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        replace: bool,
        #[arg(long, conflicts_with = "replace")]
        keep_both: bool,
    },
    /// Kill sessions left behind by a crashed app or CLI
    Cleanup,
    /// Host the engine without the app: the control socket the other commands talk to, in the
    /// foreground until Ctrl+C or SIGTERM, which stop everything it started. What runs
    /// `switchboard start` on Linux; `start` launches it in the background when it is not running
    Serve,
    /// Link this command onto your PATH and the agent skill into ~/.claude/skills, as the
    /// app's Install switchboard Command does. Run the copy inside Switchboard.app:
    /// /Applications/Switchboard.app/Contents/Helpers/switchboard setup
    Setup,
}

fn main() {
    let cli = Cli::parse();
    let projects_dir = cli.projects_dir.unwrap_or_else(project::default_projects_dir);
    let code = match cli.cmd {
        Cmd::Status { project, json } => app::status(project, json, &projects_dir),
        Cmd::Start { project, target } => app::start(&project, target),
        Cmd::Stop { project, target } => app::stop(&project, target),
        Cmd::Restart { project, target } => app::restart(&project, target),
        Cmd::Logs { command, target, tail } => app::logs(&command, target, tail),
        Cmd::Attach { project, target } => app::attach(&project, target),
        Cmd::Herd { project } => herd::herd(&project),
        Cmd::Ui => ui::run(&projects_dir),
        Cmd::Wait { command, target, until, timeout } => app::wait(&command, target, until, timeout),
        Cmd::Open { project } => app::open(project),
        Cmd::Board { dir, cmd } => board::run(&projects_dir, dir, cmd),
        Cmd::Keep { project, folder, json } => app::keep(&projects_dir, &project, folder, json),
        Cmd::EditCommand { target, run, cwd, name, env, unset_env, autostart, no_autostart, restart_on_crash, no_restart_on_crash } => {
            let flag = |on: bool, off: bool| if on { Some(true) } else if off { Some(false) } else { None };
            let edit = sb_core::CommandEdit { run, cwd, name, env: Default::default(), unset_env,
                                              autostart: flag(autostart, no_autostart), restart_on_crash: flag(restart_on_crash, no_restart_on_crash) };
            edit_command(&projects_dir, &target, &env, edit)
        }
        Cmd::AddCommand { project, id, run, name, cwd, no_autostart, restart_on_crash, env, group_name, group_cwd } => {
            add_command(&projects_dir, &project, AddArgs { id, run, name, cwd, no_autostart, restart_on_crash, env, group_name, group_cwd })
        }
        Cmd::List => list(&projects_dir),
        Cmd::Validate { path } => validate(&path),
        Cmd::Run { project, target } => run(&projects_dir, &project, target.as_deref()),
        Cmd::Install { path, replace, keep_both } => install(&projects_dir, &path, replace, keep_both),
        Cmd::Import { path, replace, keep_both } => import(&projects_dir, &path, replace, keep_both),
        Cmd::ImportSolo { paths, dry_run, replace, keep_both } => import_solo(&projects_dir, &paths, dry_run, replace, keep_both),
        Cmd::Cleanup => cleanup(&projects_dir),
        Cmd::Serve => serve(&projects_dir),
        Cmd::Setup => setup(),
    };
    match code {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("{} {e:#}", "error:".red().bold());
            std::process::exit(2);
        }
    }
}

// ----- setup ------------------------------------------------------------------

fn setup() -> Result<i32> {
    use sb_core::tools::{self, ClaudeMd};
    let exe = std::env::current_exe()?;
    let (cli, skill) = tools::bundled_from(&exe).map_err(|e| anyhow!(e))?;
    let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| anyhow!("HOME is not set"))?;
    let got = tools::install(&cli, &skill, &home, None).map_err(|e| anyhow!(e))?;
    println!("{} {}", "command".green(), got.command.display());
    match &got.skill {
        Ok(p) => println!("{} {}", "skill".green(), p.display()),
        Err(e) => println!("{} {e}", "skill".red()),
    }
    match &got.claude_md {
        Ok(ClaudeMd::Added(p)) => println!("{} added /switchboard to {}", "CLAUDE.md".green(), p.display()),
        Ok(ClaudeMd::AlreadyListed(p)) => println!("{} {} already lists /switchboard", "CLAUDE.md".dimmed(), p.display()),
        Ok(ClaudeMd::NoFile) => {}
        Err(e) => println!("{} {e}", "CLAUDE.md".red()),
    }
    if !got.on_path {
        let dir = got.command.parent().unwrap_or(Path::new(""));
        println!("{} is not on your PATH yet. Add it in your shell profile, then open a new terminal.", dir.display());
    }
    Ok(if got.skill.is_ok() { 0 } else { 1 })
}

// ----- list -------------------------------------------------------------------

pub(crate) fn list(projects_dir: &Path) -> Result<i32> {
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
        match &p.effective {
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

// ----- edit-command -------------------------------------------------------------

fn edit_command(projects_dir: &Path, target: &str, env: &[String], mut e: sb_core::CommandEdit) -> Result<i32> {
    for kv in env {
        let (k, v) = kv.split_once('=').ok_or_else(|| anyhow!("--env takes KEY=VALUE, got {kv:?}"))?;
        e.env.insert(k.to_string(), v.to_string());
    }
    let (slug, address) = app::split_target(target, None);
    let address = address.ok_or_else(|| anyhow!("name a command: project/group/command, or project/command"))?;
    if let Some(cwd) = e.cwd.as_deref().filter(|c| !c.is_empty()) {
        if !sb_core::config::expand_tilde(cwd).is_dir() {
            eprintln!("{} {cwd} does not exist yet", "warning:".yellow());
        }
    }
    let engine = Engine::new(projects_dir);
    match engine.edit_command(&slug, &address, &e) {
        Ok(changed) if changed.is_empty() => {
            println!("{} {slug}/{address} already has that", "ok".green().bold());
            Ok(0)
        }
        Ok(changed) => {
            println!("{} {slug}/{address}: {}", "changed".green().bold(), changed.join(", "));
            println!("  if it's running, apply it: switchboard restart {slug}/{address}");
            Ok(0)
        }
        Err(sb_core::EngineError::UnknownGroup(g)) => bail!("{slug} has no group {g:?}"),
        Err(err) => bail!("{err}"),
    }
}

// ----- add-command --------------------------------------------------------------

struct AddArgs {
    id: String,
    run: String,
    name: Option<String>,
    cwd: Option<String>,
    no_autostart: bool,
    restart_on_crash: bool,
    env: Vec<String>,
    group_name: Option<String>,
    group_cwd: Option<String>,
}

fn add_command(projects_dir: &Path, target: &str, a: AddArgs) -> Result<i32> {
    let (slug, group) = app::split_target(target, None);
    if group.as_deref().is_some_and(|g| g.contains('/')) {
        bail!("name a project or project/group, not a command");
    }
    let id = project::slugify(&a.id);
    let mut env = std::collections::BTreeMap::new();
    for kv in &a.env {
        let (k, v) = kv.split_once('=').ok_or_else(|| anyhow!("--env takes KEY=VALUE, got {kv:?}"))?;
        env.insert(k.to_string(), v.to_string());
    }
    for folder in [&a.cwd, &a.group_cwd].into_iter().flatten() {
        if !sb_core::config::expand_tilde(folder).is_dir() {
            eprintln!("{} {folder} does not exist yet", "warning:".yellow());
        }
    }
    let cmd: sb_core::CommandConfig = bare_command(&id, &a.run)?;
    let cmd = sb_core::CommandConfig {
        name: a.name,
        cwd: a.cwd,
        env,
        autostart: a.no_autostart.then_some(false),
        restart_on_crash: a.restart_on_crash.then_some(true),
        ..cmd
    };
    let new_group = (a.group_name.is_some() || a.group_cwd.is_some()).then(|| (a.group_name.clone(), a.group_cwd.clone()));
    let engine = Engine::new(projects_dir);
    match engine.add_command(&slug, group.as_deref(), new_group, cmd) {
        Ok(address) => {
            println!("{} added {slug}/{address}", "ok".green().bold());
            println!("  start it: switchboard start {slug}/{address}");
            Ok(0)
        }
        Err(sb_core::EngineError::UnknownGroup(g)) => {
            bail!("{slug} has no group {g:?}; pass --group-name and --group-cwd to create it")
        }
        Err(e) => bail!("{e}"),
    }
}

/// A command with just an id and run line; the rest is filled in by the caller.
fn bare_command(id: &str, run: &str) -> Result<sb_core::CommandConfig> {
    Ok(sb_core::CommandConfig {
        id: id.to_string(),
        name: None,
        run: run.to_string(),
        cwd: None,
        env: Default::default(),
        autostart: None,
        restart_on_crash: None,
        terminal: false,
        extra: Default::default(),
    })
}

// ----- validate ---------------------------------------------------------------

fn validate(path: &Path) -> Result<i32> {
    use sb_core::fragment::{DOT_DIR, PROJECT_FILE};
    // A folder with config.yaml, a repo folder with .switchboard/project.yaml, or a file.
    let file = if path.is_dir() {
        let seed = path.join(DOT_DIR).join(PROJECT_FILE);
        if !path.join(project::CONFIG_FILE).is_file() && seed.is_file() { seed } else { path.join(project::CONFIG_FILE) }
    } else {
        path.to_path_buf()
    };
    let is_seed = file.file_name().map(|n| n == PROJECT_FILE).unwrap_or(false)
        && file.parent().and_then(|p| p.file_name()).map(|n| n == DOT_DIR).unwrap_or(false);
    match ProjectConfig::load(&file) {
        Ok(raw) => {
            // A seed's relative paths resolve from its home folder (the one holding .switchboard).
            let base = if is_seed {
                file.parent().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default()
            } else {
                file.parent().map(Path::to_path_buf).unwrap_or_default()
            };
            // Check a seed as it installs, with group folders resolved from its home.
            let raw = if is_seed {
                sb_core::fragment::ProjectSeed { updated: None, config: raw }.to_config(&base)
            } else {
                raw
            };
            let composed = raw.compose(&base);
            let cfg = composed.config;
            for pr in &composed.problems {
                println!("  {} group {}: {}", "warning".yellow(), pr.group, pr.message);
            }
            for o in &composed.overlays {
                println!("  {} {}", "overlay".dimmed(), sb_core::config::collapse_tilde(o));
            }
            for src in &composed.sources {
                if src.is_file() && !composed.overlays.contains(src) {
                    println!("  {} {}", "fragment".dimmed(), sb_core::config::collapse_tilde(src));
                }
            }
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

fn import(projects_dir: &Path, path: &Path, replace: bool, keep_both: bool) -> Result<i32> {
    let found = sb_core::fragment::discover_seeds(path);
    let Some(best) = found.first() else {
        eprintln!("no .switchboard/project.yaml in {} (or the folders it references)", path.display());
        return Ok(1);
    };
    if found.len() > 1 {
        println!("found {} project.yaml files; using the newest:", found.len());
        for c in &found {
            println!("  {} {}", if std::ptr::eq(c, best) { "*" } else { " " }, sb_core::config::collapse_tilde(&c.home));
        }
    }
    for (group, folder, exists) in &best.members {
        let mark = if *exists { "ok".green().to_string() } else { "missing".yellow().to_string() };
        println!("  {group:<20} {} {mark}", sb_core::config::collapse_tilde(folder));
    }
    let conflict = if replace { Conflict::Replace } else if keep_both { Conflict::KeepBoth } else { Conflict::Fail };
    match sb_core::fragment::install_seed(best, projects_dir, conflict) {
        Ok(dest) => {
            println!("{} imported {} as {}", "ok".green().bold(), best.seed.config.name, dest.display());
            Ok(0)
        }
        Err(project::InstallError::Exists(slug)) => {
            eprintln!("a project folder named {slug:?} already exists; pass --replace or --keep-both");
            Ok(1)
        }
        Err(e) => {
            eprintln!("{} {e}", "not imported:".red().bold());
            Ok(1)
        }
    }
}

fn import_solo(projects_dir: &Path, paths: &[PathBuf], dry_run: bool, replace: bool, keep_both: bool) -> Result<i32> {
    use sb_core::solo;
    let files: Vec<PathBuf> = paths.iter().flat_map(|p| if p.is_file() { vec![p.clone()] } else { solo::find_solo_files(p) }).collect();
    if files.is_empty() {
        eprintln!("no solo.yml found in {}", paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "));
        return Ok(1);
    }
    let import = match solo::convert(&files) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("{} {e}", "could not convert:".red().bold());
            return Ok(1);
        }
    };
    for f in &import.sources {
        println!("  {} {}", "read".dimmed(), sb_core::config::collapse_tilde(f));
    }
    for g in &import.config.groups {
        println!("  {:<22} {}  {}", g.display_name(), g.cwd.clone().unwrap_or_default().dimmed(),
                 g.commands.iter().map(|c| c.display_name().to_string()).collect::<Vec<_>>().join(", "));
    }
    for n in &import.notes {
        println!("  {} {n}", "note".yellow());
    }
    if dry_run {
        println!("\n{}", import.config.to_yaml().map_err(|e| anyhow!("{e}"))?);
        return Ok(0);
    }
    let conflict = if replace { Conflict::Replace } else if keep_both { Conflict::KeepBoth } else { Conflict::Fail };
    match solo::install(&import, projects_dir, conflict) {
        Ok(dest) => {
            println!("{} converted {} to {}", "ok".green().bold(), import.config.name, dest.display());
            Ok(0)
        }
        Err(project::InstallError::Exists(slug)) => {
            eprintln!("a project folder named {slug:?} already exists; pass --replace or --keep-both");
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

// ----- serve ----------------------------------------------------------------------

/// What the Mac app does at launch and quit, without the UI: the engine, its file watcher
/// and the control socket, then stop everything on the way out.
fn serve(projects_dir: &Path) -> Result<i32> {
    let engine = Engine::new(projects_dir);
    // Sessions left by a host that died without stopping them. Unattended there is no one
    // to ask, as the app does, and they would hold the ports their commands need.
    let leftovers = engine.leftover_sessions();
    if !leftovers.is_empty() {
        for e in &leftovers {
            println!("killing leftover {}:{} (pid {})", e.project, e.address, e.pid);
        }
        engine.kill_leftovers();
    }
    if let Err(e) = engine.watch() {
        eprintln!("{} file watcher: {e}", "warning".yellow());
    }
    let sock = sb_core::control::socket_path();
    let server = sb_core::control::serve(&engine, &sock, None).map_err(|e| anyhow!(e))?;
    install_signal_handlers();
    println!(
        "serving {} on {} (Ctrl+C stops everything)",
        sb_core::config::collapse_tilde(engine.projects_dir()),
        sb_core::config::collapse_tilde(&sock)
    );
    while !INTERRUPTED.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(200));
    }
    // Stop answering first, so nothing starts while everything stops.
    drop(server);
    if engine.any_alive() {
        println!("stopping…");
        engine.stop_everything_blocking();
    }
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
    // `switchboard run path/to/folder` runs a project folder directly.
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
    let cfg = match &proj.effective {
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
        format!("{} {} ", format!("{name:>w$}", w = self.width).styled(style), "│".dimmed())
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
            s.push_str(&format!("{} {}\r\n", "switchboard:".dimmed(), text));
        } else {
            s.push_str(&format!("{}{}\n", self.prefix(address), text));
        }
        self.at_line_start.insert(address.to_string(), true);
        let _ = out.write_all(s.as_bytes());
        let _ = out.flush();
    }

    fn note(&self, text: &str) {
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(format!("\r\n{} {}\r\n", "switchboard:".dimmed(), text).as_bytes());
        let _ = out.flush();
    }
}
