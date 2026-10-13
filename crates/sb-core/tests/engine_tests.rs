//! Spawns real processes. Each test uses its own temporary Projects directory.

use sb_core::engine::{Engine, Event, Status};
use sb_core::session::CommandKey;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn write_project(projects: &std::path::Path, slug: &str, yaml: &str) {
    let dir = projects.join(slug);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.yaml"), yaml).unwrap();
}

fn wait_for(engine: &Engine, key: &CommandKey, pred: impl Fn(&sb_core::CommandState) -> bool, timeout: Duration) -> sb_core::CommandState {
    let deadline = Instant::now() + timeout;
    loop {
        let st = engine.state_of(key);
        if pred(&st) {
            return st;
        }
        if Instant::now() > deadline {
            panic!("timed out waiting; last state {st:?}");
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}

fn pids_matching(marker: &str) -> Vec<String> {
    let out = std::process::Command::new("ps").args(["-axo", "pid=,command="]).output().unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.contains(marker) && !l.contains("ps -axo"))
        .map(|l| l.to_string())
        .collect()
}

#[test]
fn starts_streams_output_and_reports_clean_exit() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("Projects");
    write_project(&projects, "demo", "version: 1\nname: Demo\ngroups:\n  - id: g\n    cwd: /tmp\n    commands:\n      - id: hello\n        run: pwd; echo -e '\\e[32mhello\\e[0m'; exit 0\n");
    let engine = Engine::new(&projects);
    let output = Arc::new(Mutex::new(Vec::<u8>::new()));
    let statuses = Arc::new(Mutex::new(Vec::<Status>::new()));
    let (o, s) = (output.clone(), statuses.clone());
    engine.subscribe(Arc::new(move |e: &Event| match e {
        Event::Output { bytes, .. } => o.lock().unwrap().extend_from_slice(bytes),
        Event::Status { state, .. } => s.lock().unwrap().push(state.status.clone()),
        _ => {}
    }));

    let key = CommandKey::new("demo", "g/hello");
    engine.start(&key).unwrap();
    let st = wait_for(&engine, &key, |s| s.status == Status::Exited, Duration::from_secs(10));
    assert_eq!(st.exit_code, Some(0));
    assert!(st.pid.is_some());
    let text = String::from_utf8_lossy(&output.lock().unwrap()).to_string();
    assert!(text.contains("\x1b[32mhello"), "colors pass through: {text:?}");
    // /tmp is a symlink to /private/tmp on macOS; pwd reports either.
    assert!(text.contains("/tmp"), "ran in the group cwd: {text:?}");
    let seen = statuses.lock().unwrap().clone();
    assert_eq!(seen.first(), Some(&Status::Starting));
    assert_eq!(seen.last(), Some(&Status::Exited));
    assert!(engine.screen_text(&key).contains("hello"));
}

#[test]
fn nonzero_exit_is_crashed_and_missing_folder_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("Projects");
    write_project(&projects, "demo", "version: 1\nname: Demo\ncommands:\n  - id: fail\n    run: exit 3\n  - id: nowhere\n    run: echo hi\n    cwd: /definitely/not/here\n");
    let engine = Engine::new(&projects);
    let fail = CommandKey::new("demo", "fail");
    engine.start(&fail).unwrap();
    let st = wait_for(&engine, &fail, |s| s.status == Status::Crashed, Duration::from_secs(10));
    assert_eq!(st.exit_code, Some(3));

    let nowhere = CommandKey::new("demo", "nowhere");
    let err = engine.start(&nowhere).unwrap_err();
    assert!(err.to_string().contains("Folder not found"), "{err}");
    let st = engine.state_of(&nowhere);
    assert_eq!(st.status, Status::Error);
    assert!(st.message.unwrap().contains("/definitely/not/here"));
    assert!(st.pid.is_none());
}

#[test]
fn stop_kills_the_whole_process_tree() {
    let marker = format!("sbtest-{}", std::process::id());
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("Projects");
    // A shell that traps SIGINT/SIGTERM and spawns grandchildren that also ignore them,
    // so only the SIGKILL to the process group can end the tree.
    let run = format!(
        "trap '' INT TERM; (trap '' INT TERM; sleep 600; echo {marker}) & (trap '' INT TERM; sleep 600; echo {marker}) & sleep 600; echo {marker}"
    );
    write_project(&projects, "demo", &format!("version: 1\nname: Demo\ncommands:\n  - id: tree\n    run: \"{run}\"\n"));
    let engine = Engine::new(&projects);
    let key = CommandKey::new("demo", "tree");
    engine.start(&key).unwrap();
    wait_for(&engine, &key, |s| s.status == Status::Running, Duration::from_secs(10));
    std::thread::sleep(Duration::from_millis(600));
    let before = pids_matching(&marker);
    assert!(before.len() >= 3, "expected a process tree, saw {before:?}");
    let pid = engine.state_of(&key).pid.unwrap();
    assert!(sb_core::session::pgid_alive(pid));

    let t = Instant::now();
    engine.stop_blocking(&key);
    let st = wait_for(&engine, &key, |s| !s.is_alive(), Duration::from_secs(10));
    assert_eq!(st.status, Status::Stopped, "a requested stop is never a crash");
    assert!(st.signal.is_some());
    // SIGINT ignored (3 s), SIGTERM ignored (3 s), SIGKILL ends it: under ~9 s.
    assert!(t.elapsed() < Duration::from_secs(9), "took {:?}", t.elapsed());
    std::thread::sleep(Duration::from_millis(300));
    let after = pids_matching(&marker);
    assert!(after.is_empty(), "orphans left behind: {after:?}");
    assert!(!sb_core::session::pgid_alive(pid));
}

#[test]
fn polite_process_stops_on_sigint_quickly() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("Projects");
    write_project(&projects, "demo", "version: 1\nname: Demo\ncommands:\n  - id: sleepy\n    run: sleep 600\n");
    let engine = Engine::new(&projects);
    let key = CommandKey::new("demo", "sleepy");
    engine.start(&key).unwrap();
    wait_for(&engine, &key, |s| s.status == Status::Running, Duration::from_secs(10));
    let t = Instant::now();
    engine.stop_blocking(&key);
    assert!(t.elapsed() < Duration::from_secs(2), "SIGINT should be enough: {:?}", t.elapsed());
    let st = wait_for(&engine, &key, |s| !s.is_alive(), Duration::from_secs(5));
    assert!(!st.is_alive());
    assert!(!engine.any_alive());
}

#[test]
fn restart_on_crash_backs_off_and_stop_cancels_it() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("Projects");
    write_project(&projects, "demo", "version: 1\nname: Demo\ncommands:\n  - id: flaky\n    run: exit 1\n    restart_on_crash: true\n");
    let engine = Engine::new(&projects);
    let key = CommandKey::new("demo", "flaky");
    let starts = Arc::new(Mutex::new(0u32));
    let s = starts.clone();
    engine.subscribe(Arc::new(move |e: &Event| {
        if let Event::Status { state, .. } = e {
            if state.status == Status::Starting && state.pid.is_none() {
                *s.lock().unwrap() += 1;
            }
        }
    }));
    engine.start(&key).unwrap();
    let st = wait_for(&engine, &key, |s| s.status == Status::Crashed, Duration::from_secs(10));
    assert_eq!(st.message.as_deref(), Some("crashed, restarting in 1 s"));
    assert!(st.restart_at.is_some(), "backoff deadline is reported");
    // After ~1 s it restarts (start #2), crashes, schedules 2 s (start #3 at ~3 s).
    wait_for(&engine, &key, |s| s.message.as_deref() == Some("crashed, restarting in 2 s"), Duration::from_secs(5));
    assert_eq!(*starts.lock().unwrap(), 2);
    engine.stop(&key);
    std::thread::sleep(Duration::from_millis(2600));
    assert_eq!(*starts.lock().unwrap(), 2, "stop must cancel the pending restart");
    assert_eq!(engine.state_of(&key).message, None);
}

#[test]
fn start_all_runs_only_autostart_commands_and_stop_everything_clears_them() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("Projects");
    write_project(&projects, "demo", "version: 1\nname: Demo\ngroups:\n  - id: a\n    commands:\n      - id: one\n        run: sleep 600\n      - id: two\n        run: sleep 600\n        autostart: false\n  - id: b\n    commands:\n      - id: three\n        run: sleep 600\n");
    let engine = Engine::new(&projects);
    let started = engine.start_all("demo", None).unwrap();
    let addrs: Vec<&str> = started.iter().map(|k| k.address.as_str()).collect();
    assert_eq!(addrs, ["a/one", "b/three"]);
    for k in &started {
        wait_for(&engine, k, |s| s.status == Status::Running, Duration::from_secs(10));
    }
    assert_eq!(engine.state_of(&CommandKey::new("demo", "a/two")).status, Status::Stopped);

    let only_b = engine.start_all("demo", Some("b")).unwrap();
    assert_eq!(only_b.len(), 1);
    assert!(engine.start_all("demo", Some("zzz")).is_err());

    engine.stop_everything_blocking();
    assert!(!engine.any_alive());
    for k in &started {
        assert!(!engine.state_of(k).is_alive());
    }
}

#[test]
fn reload_flags_sessions_whose_config_changed() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("Projects");
    write_project(&projects, "demo", "version: 1\nname: Demo\ncommands:\n  - id: s\n    run: sleep 600\n");
    let engine = Engine::new(&projects);
    let key = CommandKey::new("demo", "s");
    engine.start(&key).unwrap();
    wait_for(&engine, &key, |s| s.status == Status::Running, Duration::from_secs(10));
    write_project(&projects, "demo", "version: 1\nname: Demo Renamed\ncommands:\n  - id: s\n    run: sleep 601\n");
    engine.reload();
    assert_eq!(engine.project("demo").unwrap().name(), "Demo Renamed");
    let st = engine.state_of(&key);
    assert!(st.is_alive(), "reload must not kill sessions");
    assert!(st.needs_restart);
    engine.stop_everything_blocking();
}

#[test]
fn input_reaches_the_process_and_resize_is_accepted() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("Projects");
    write_project(&projects, "demo", "version: 1\nname: Demo\ncommands:\n  - id: cat\n    run: cat\n");
    let engine = Engine::new(&projects);
    let out = Arc::new(Mutex::new(Vec::<u8>::new()));
    let o = out.clone();
    engine.subscribe(Arc::new(move |e: &Event| {
        if let Event::Output { bytes, .. } = e {
            o.lock().unwrap().extend_from_slice(bytes);
        }
    }));
    let key = CommandKey::new("demo", "cat");
    engine.start(&key).unwrap();
    wait_for(&engine, &key, |s| s.status == Status::Running, Duration::from_secs(10));
    engine.resize(&key, 100, 40);
    assert_eq!(engine.session(&key).unwrap().size(), (100, 40));
    engine.write_input(&key, b"ping\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if String::from_utf8_lossy(&out.lock().unwrap()).matches("ping").count() >= 2 {
            break; // echo + cat's output
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    let text = String::from_utf8_lossy(&out.lock().unwrap()).to_string();
    assert!(text.matches("ping").count() >= 2, "{text:?}");
    engine.stop_everything_blocking();
    assert!(engine.write_input(&key, b"x").is_err());
}
