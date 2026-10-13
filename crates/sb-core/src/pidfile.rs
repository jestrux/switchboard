//! Records live sessions so a later launch (or `sb list`) can see them.

use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

pub const FILE_NAME: &str = "sessions.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub project: String,
    pub address: String,
    pub pid: u32,
    pub started_at: u64,
}

pub fn read(path: &Path) -> Vec<Entry> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn write(path: &Path, entries: &[Entry]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(entries).unwrap_or_default())?;
    std::fs::rename(tmp, path)
}

/// Entries whose process group is still alive.
pub fn alive(entries: &[Entry]) -> Vec<Entry> {
    entries
        .iter()
        .filter(|e| crate::session::pgid_alive(e.pid))
        .cloned()
        .collect()
}

/// SIGTERM a process group, then SIGKILL it after 3 s if anything is left.
pub fn kill_group(pgid: u32) {
    let pid = Pid::from_raw(pgid as i32);
    let _ = killpg(pid, Signal::SIGTERM);
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if killpg(pid, None).is_err() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = killpg(pid, Signal::SIGKILL);
}
