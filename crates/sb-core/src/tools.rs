//! Installing the `switchboard` command and its agent skill from the app bundle.
//!
//! Both are symlinks into the bundle, so updating the app updates them. Nothing that is
//! not already one of our links is ever replaced. The skill is also listed in
//! `~/.claude/CLAUDE.md` when that file exists and does not mention it yet.

use std::path::{Path, PathBuf};

pub const COMMAND_NAME: &str = "switchboard";
pub const SKILL_NAME: &str = "switchboard";

#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    /// Where the command was linked.
    pub command: PathBuf,
    /// The folder is on the login shell's PATH, so `switchboard` works in a new terminal.
    pub on_path: bool,
    /// Where the skill was linked, or why it was not.
    pub skill: Result<PathBuf, String>,
    /// What happened to `~/.claude/CLAUDE.md`, or why it could not be updated.
    pub claude_md: Result<ClaudeMd, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClaudeMd {
    /// The skill was added to this file.
    Added(PathBuf),
    /// The file already mentions the skill.
    AlreadyListed(PathBuf),
    /// There is no `~/.claude/CLAUDE.md`; the skill is still found in `~/.claude/skills`.
    NoFile,
}

/// The row added to a skills table, and the section added when there is no table.
const SKILL_ROW: &str = "| `/switchboard` | Run, wait on and read a project's dev servers in the Switchboard app; add commands to a project; set a product up as a project |";
const SKILL_SECTION: &str = "## Switchboard\n\nUse the `/switchboard` skill to run, wait on and read a project's dev servers in the\nSwitchboard app, add commands to a project and set a product up as a project.\n";

/// Link `cli` (the bundled binary) onto the PATH and `skill` (the bundled skill folder)
/// into `~/.claude/skills`. `path` is the login shell's PATH; `None` asks the shell.
pub fn install(cli: &Path, skill: &Path, home: &Path, path: Option<&str>) -> Result<Installed, String> {
    if !cli.is_file() {
        return Err(format!("the app has no {} command at {}", COMMAND_NAME, cli.display()));
    }
    let path = match path {
        Some(p) => p.to_string(),
        None => login_path().unwrap_or_default(),
    };
    let entries: Vec<PathBuf> = std::env::split_paths(&path).collect();
    let (dir, on_path) = pick_bin_dir(home, &entries);
    std::fs::create_dir_all(&dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    let command = dir.join(COMMAND_NAME);
    link(cli, &command)?;
    let skill = if skill.join("SKILL.md").is_file() {
        let dest = home.join(".claude/skills").join(SKILL_NAME);
        std::fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string()).and_then(|_| link(skill, &dest).map(|_| dest))
    } else {
        Err(format!("the app has no skill at {}", skill.display()))
    };
    let claude_md = match &skill {
        Ok(_) => list_skill(&home.join(".claude/CLAUDE.md")),
        Err(_) => Ok(ClaudeMd::NoFile),
    };
    Ok(Installed { command, on_path, skill, claude_md })
}

/// Mention the skill in the author's CLAUDE.md: a row after the last row of its skills
/// table (a line starting with a pipe and a backticked slash name), or a short section at
/// the end. Never creates the file, and leaves it alone when it already names the skill.
pub fn list_skill(file: &Path) -> Result<ClaudeMd, String> {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(ClaudeMd::NoFile),
        Err(e) => return Err(format!("could not read {}: {e}", file.display())),
    };
    if text.contains(&format!("`/{SKILL_NAME}`")) {
        return Ok(ClaudeMd::AlreadyListed(file.to_path_buf()));
    }
    std::fs::write(file, with_skill(&text)).map_err(|e| format!("could not update {}: {e}", file.display()))?;
    Ok(ClaudeMd::Added(file.to_path_buf()))
}

fn with_skill(text: &str) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let is_row = |l: &str| l.trim_start().starts_with("| `/");
    if let Some(last) = lines.iter().rposition(|l| is_row(l)) {
        // Match the table's column count: our two cells, then empty ones.
        let cells = lines[last].trim().matches('|').count().saturating_sub(1);
        let mut row = SKILL_ROW.to_string();
        for _ in 2..cells {
            row.push_str("  |");
        }
        let mut out = String::new();
        for (i, l) in lines.iter().enumerate() {
            out.push_str(l);
            if i == last {
                if !l.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(&row);
                out.push('\n');
            }
        }
        return out;
    }
    let mut out = text.trim_end().to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(SKILL_SECTION);
    out
}

/// The first user-writable bin folder on the PATH, preferring the usual ones; otherwise
/// `~/.local/bin`, which then still has to be added to the PATH.
fn pick_bin_dir(home: &Path, path: &[PathBuf]) -> (PathBuf, bool) {
    let preferred = [
        home.join(".local/bin"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        home.join("bin"),
    ];
    for dir in &preferred {
        if path.iter().any(|p| same_dir(p, dir)) && writable(dir) {
            return (dir.clone(), true);
        }
    }
    (preferred[0].clone(), false)
}

fn same_dir(a: &Path, b: &Path) -> bool {
    let trim = |p: &Path| p.to_string_lossy().trim_end_matches('/').to_string();
    trim(a) == trim(b)
}

fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".switchboard-probe-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// Point `dest` at `target`, replacing only an earlier symlink.
fn link(target: &Path, dest: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(dest) {
        Ok(m) if m.file_type().is_symlink() => {
            std::fs::remove_file(dest).map_err(|e| format!("could not replace {}: {e}", dest.display()))?;
        }
        Ok(_) => return Err(format!("{} already exists and is not a link Switchboard made; move it away first", dest.display())),
        Err(_) => {}
    }
    std::os::unix::fs::symlink(target, dest).map_err(|e| format!("could not link {}: {e}", dest.display()))
}

/// The PATH a new terminal gets: the login shell's, rc files included.
pub fn login_path() -> Option<String> {
    const MARK: &str = "__SWITCHBOARD_PATH__";
    let out = std::process::Command::new(crate::session::login_shell())
        .args(["-ilc", &format!("printf '{MARK}%s' \"$PATH\"")])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.rsplit_once(MARK).map(|(_, p)| p.trim().to_string())
}
