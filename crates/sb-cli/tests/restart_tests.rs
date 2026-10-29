//! `wait` through a crash restart. Its own file (its own process), because it points
//! `SWITCHBOARD_HOME` at its own support folder.

use sb_core::control;
use sb_core::engine::Engine;
use std::process::Command;

fn sb(home: &std::path::Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_switchboard"))
        .args(args)
        .env("SWITCHBOARD_HOME", home)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn wait_rides_through_a_scheduled_crash_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    std::env::set_var("SWITCHBOARD_HOME", home);
    let projects = home.join("Projects");
    std::fs::create_dir_all(projects.join("flaky")).unwrap();
    // Crashes on its first run, serves on the second: what a dev server killed under it does.
    let marker = home.join("ran-once");
    let run = format!(
        "if [ -f {m} ]; then echo listening on http://localhost:47813; sleep 30; else touch {m}; exit 1; fi",
        m = marker.display()
    );
    std::fs::write(
        projects.join("flaky/config.yaml"),
        format!("version: 1\nname: Flaky\ngroups:\n  - id: web\n    cwd: /tmp\n    commands:\n      - id: server\n        run: '{run}'\n        restart_on_crash: true\n"),
    )
    .unwrap();
    let engine = Engine::new(&projects);
    let _server = control::serve(&engine, &control::socket_path(), None).unwrap();

    let (code, out, err) = sb(home, &["start", "flaky"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("started web/server"), "{out}");
    let (code, out, err) = sb(home, &["wait", "flaky/web/server", "--timeout", "15"]);
    assert_eq!(code, 0, "wait gave up on a command that was restarting:\n{out}\n{err}");
    assert!(out.contains("ready flaky/web/server  http://localhost:47813"), "{out}");
    assert!(marker.exists(), "the first run should have crashed");

    sb(home, &["stop", "flaky"]);
}
