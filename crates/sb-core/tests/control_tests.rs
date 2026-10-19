//! The control socket, driven through the same client the `switchboard` command uses.

use sb_core::control::{self, Client, ControlHandler, Request};
use sb_core::engine::Engine;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Keep the engine's sessions.json out of the real app support folder.
fn isolate() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let dir = std::env::temp_dir().join(format!("sb-tests-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("SWITCHBOARD_HOME", dir);
    });
}

fn write_project(projects: &std::path::Path, slug: &str, yaml: &str) {
    isolate();
    let dir = projects.join(slug);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.yaml"), yaml).unwrap();
}

const DEMO: &str = "version: 1\nname: Demo\ngroups:\n  - id: g\n    cwd: /tmp\n    commands:\n      - id: server\n        run: \"echo listening on http://localhost:47811; sleep 30\"\n        autostart: true\n      - id: lines\n        run: \"for i in $(seq 1 200); do echo line $i; done; sleep 30\"\n        autostart: false\n      - id: once\n        run: \"echo done\"\n        autostart: false\n";

fn command<'a>(status: &'a serde_json::Value, address: &str) -> &'a serde_json::Value {
    status["projects"][0]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["address"] == address)
        .unwrap()
}

fn wait_status(client: &mut Client, address: &str, pred: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let st = client.call(&Request::Status { project: Some("demo".into()) }).unwrap();
        let c = command(&st, address).clone();
        if pred(&c) {
            return c;
        }
        assert!(Instant::now() < deadline, "timed out; last {c}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn starts_reports_reads_and_stops_through_the_socket() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("Projects");
    write_project(&projects, "demo", DEMO);
    let engine = Engine::new(&projects);
    let sock = tmp.path().join("c.sock");
    let _server = control::serve(&engine, &sock, None).unwrap();
    let mut client = Client::connect(&sock).unwrap();

    assert_eq!(client.call(&Request::Ping).unwrap()["app"], "Switchboard");

    // No target: the autostart commands only.
    let started = client.call(&Request::Start { project: "demo".into(), target: None }).unwrap();
    let addrs: Vec<&str> = started["commands"].as_array().unwrap().iter().map(|c| c["address"].as_str().unwrap()).collect();
    assert_eq!(addrs, ["g/server"]);
    let server = wait_status(&mut client, "g/server", |c| c["port"] == 47811);
    assert_eq!(server["status"], "running");
    assert_eq!(server["url"], "http://localhost:47811");

    // A named command starts even though it is not autostart; logs reach into scrollback.
    client.call(&Request::Start { project: "demo".into(), target: Some("g/lines".into()) }).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let text = loop {
        let v = client.call(&Request::Logs { project: "demo".into(), address: "g/lines".into(), lines: 150 }).unwrap();
        let t = v["text"].as_str().unwrap().to_string();
        if t.contains("line 200") || Instant::now() > deadline {
            break t;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 150, "{text}");
    assert_eq!(lines.first(), Some(&"line 51"));
    assert_eq!(lines.last(), Some(&"line 200"));

    // Stop waits until everything has exited.
    let stopped = client.call(&Request::Stop { project: "demo".into(), target: None }).unwrap();
    assert_eq!(stopped["commands"].as_array().unwrap().len(), 2);
    let st = client.call(&Request::Status { project: Some("demo".into()) }).unwrap();
    assert_eq!(command(&st, "g/server")["status"], "stopped");
    assert_eq!(command(&st, "g/lines")["status"], "stopped");
}

#[test]
fn errors_come_back_as_messages() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("Projects");
    write_project(&projects, "demo", DEMO);
    let engine = Engine::new(&projects);
    let sock = tmp.path().join("c.sock");
    let _server = control::serve(&engine, &sock, None).unwrap();
    let mut client = Client::connect(&sock).unwrap();

    let e = client.call(&Request::Start { project: "nope".into(), target: None }).unwrap_err();
    assert!(e.contains("unknown project"), "{e}");
    let e = client.call(&Request::Start { project: "demo".into(), target: Some("g/missing".into()) }).unwrap_err();
    assert!(e.contains("no command or group"), "{e}");
    let e = client.call(&Request::Logs { project: "demo".into(), address: "g/once".into(), lines: 10 }).unwrap_err();
    assert!(e.contains("has not run"), "{e}");
    let e = client.call(&Request::Open { project: None }).unwrap_err();
    assert!(e.contains("no window"), "{e}");

    // A second app cannot take over a socket that is being served.
    assert!(control::serve(&engine, &sock, None).is_err());
}

#[test]
fn a_finished_command_reports_its_exit_and_restart_runs_it_again() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("Projects");
    write_project(&projects, "demo", DEMO);
    let engine = Engine::new(&projects);
    let sock = tmp.path().join("c.sock");
    let _server = control::serve(&engine, &sock, None).unwrap();
    let mut client = Client::connect(&sock).unwrap();

    client.call(&Request::Start { project: "demo".into(), target: Some("g/once".into()) }).unwrap();
    let first = wait_status(&mut client, "g/once", |c| c["status"] == "exited");
    assert_eq!(first["exit_code"], 0);
    client.call(&Request::Restart { project: "demo".into(), target: Some("g/once".into()) }).unwrap();
    let second = wait_status(&mut client, "g/once", |c| c["status"] == "exited");
    assert!(second["started_at"].as_f64() > first["started_at"].as_f64());
}

#[test]
fn open_reaches_the_app_and_a_stale_socket_is_replaced() {
    struct Recorder(Mutex<Vec<Option<String>>>);
    impl ControlHandler for Recorder {
        fn open_project(&self, project: Option<String>) {
            self.0.lock().unwrap().push(project);
        }
    }
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("Projects");
    write_project(&projects, "demo", DEMO);
    let engine = Engine::new(&projects);
    let sock = tmp.path().join("c.sock");
    // Left behind by a crash: a socket file nobody serves.
    drop(std::os::unix::net::UnixListener::bind(&sock).unwrap());
    assert!(sock.exists());

    let rec = Arc::new(Recorder(Mutex::new(Vec::new())));
    let server = control::serve(&engine, &sock, Some(rec.clone())).unwrap();
    let mut client = Client::connect(&sock).unwrap();
    client.call(&Request::Open { project: Some("demo".into()) }).unwrap();
    assert!(client.call(&Request::Open { project: Some("other".into()) }).is_err());
    assert_eq!(*rec.0.lock().unwrap(), vec![Some("demo".to_string())]);

    drop(server);
    assert!(!sock.exists(), "dropping the server removes the socket");
    assert!(Client::connect(&sock).is_none());
}

#[test]
fn tools_link_into_a_folder_on_the_path_and_replace_only_their_own_links() {
    use sb_core::tools;
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let bundle = tmp.path().join("Switchboard.app/Contents");
    std::fs::create_dir_all(bundle.join("Helpers")).unwrap();
    std::fs::create_dir_all(bundle.join("Resources/skills/switchboard")).unwrap();
    let cli = bundle.join("Helpers/switchboard");
    let skill = bundle.join("Resources/skills/switchboard");
    std::fs::write(&cli, b"#!/bin/sh\n").unwrap();
    std::fs::write(skill.join("SKILL.md"), b"---\nname: switchboard\n---\n").unwrap();
    let bin = home.join(".local/bin");

    // ~/.local/bin on the PATH: used, and reported as on the PATH.
    std::fs::create_dir_all(&bin).unwrap();
    let path = format!("/usr/bin:{}", bin.display());
    let got = tools::install(&cli, &skill, &home, Some(&path)).unwrap();
    assert_eq!(got.command, bin.join("switchboard"));
    assert!(got.on_path);
    assert_eq!(std::fs::read_link(&got.command).unwrap(), cli);
    let skill_link = got.skill.unwrap();
    assert_eq!(skill_link, home.join(".claude/skills/switchboard"));
    assert!(skill_link.join("SKILL.md").is_file());

    // Again: our own links are replaced without complaint.
    assert!(tools::install(&cli, &skill, &home, Some(&path)).is_ok());

    // Not on the PATH: still linked into ~/.local/bin, flagged.
    let got = tools::install(&cli, &skill, &home, Some("/usr/bin")).unwrap();
    assert!(!got.on_path);

    // Someone else's file is left alone.
    std::fs::remove_file(bin.join("switchboard")).unwrap();
    std::fs::write(bin.join("switchboard"), b"theirs").unwrap();
    let e = tools::install(&cli, &skill, &home, Some(&path)).unwrap_err();
    assert!(e.contains("not a link Switchboard made"), "{e}");
    assert_eq!(std::fs::read(bin.join("switchboard")).unwrap(), b"theirs");
}
