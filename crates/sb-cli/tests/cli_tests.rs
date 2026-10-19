//! The `switchboard` binary against a served engine, as an agent would use it.
//! `SWITCHBOARD_HOME` points both at a temporary support folder.

use sb_core::control;
use sb_core::engine::Engine;
use std::process::Command;

const DEMO: &str = "version: 1\nname: Demo\ngroups:\n  - id: web\n    cwd: /tmp\n    commands:\n      - id: server\n        run: \"echo booting; sleep 0.5; echo listening on http://localhost:47812; sleep 30\"\n      - id: test\n        run: \"echo running tests; exit 3\"\n        autostart: false\n      - id: quiet\n        run: \"sleep 30\"\n        autostart: false\n";

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
fn an_agent_starts_waits_reads_and_stops() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    std::env::set_var("SWITCHBOARD_HOME", home);
    let projects = home.join("Projects");
    std::fs::create_dir_all(projects.join("demo")).unwrap();
    std::fs::write(projects.join("demo/config.yaml"), DEMO).unwrap();
    let engine = Engine::new(&projects);

    // No app yet: stop is a no-op, logs explains.
    let (code, out, _) = sb(home, &["stop", "demo"]);
    assert_eq!((code, out.trim()), (0, "Switchboard is not running; nothing to stop."));
    let (code, _, err) = sb(home, &["logs", "demo/web/server"]);
    assert_eq!(code, 2);
    assert!(err.contains("Switchboard is not running"), "{err}");

    let _server = control::serve(&engine, &control::socket_path(), None).unwrap();

    let (code, out, _) = sb(home, &["start", "demo"]);
    assert_eq!(code, 0);
    assert_eq!(out.trim(), "started web/server");

    let (code, out, err) = sb(home, &["wait", "demo/web/server", "--timeout", "10"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("ready demo/web/server  http://localhost:47812"), "{out}");

    let (_, out, _) = sb(home, &["status", "demo"]);
    assert!(out.contains("web/server") && out.contains("running  http://localhost:47812"), "{out}");
    let (_, json, _) = sb(home, &["status", "demo", "--json"]);
    assert!(json.contains("\"port\": 47812"), "{json}");

    let (_, out, _) = sb(home, &["logs", "demo/web/server", "-n", "1"]);
    assert_eq!(out.trim(), "listening on http://localhost:47812");

    // A one-off task: wait --for exit passes its exit code through and shows the output.
    sb(home, &["start", "demo/web/test"]);
    let (code, _, err) = sb(home, &["wait", "demo/web/test", "--for", "exit", "--timeout", "10"]);
    assert_eq!(code, 3, "{err}");
    assert!(err.contains("failed: demo/web/test crashed with exit code 3"), "{err}");
    assert!(err.contains("running tests"), "{err}");

    // Never prints a URL: a ready wait times out with a hint.
    sb(home, &["start", "demo", "web/quiet"]);
    let (code, _, err) = sb(home, &["wait", "demo/web/quiet", "--timeout", "1"]);
    assert_eq!(code, 124, "{err}");
    assert!(err.contains("--for running"), "{err}");
    let (code, _, _) = sb(home, &["wait", "demo/web/quiet", "--for", "running"]);
    assert_eq!(code, 0);

    let (code, out, _) = sb(home, &["stop", "demo"]);
    assert_eq!(code, 0);
    assert!(out.contains("stopped web/server") && out.contains("stopped web/quiet"), "{out}");
    let (code, _, err) = sb(home, &["wait", "demo/web/server"]);
    assert_eq!(code, 2);
    assert!(err.contains("is not running"), "{err}");

    let (code, _, err) = sb(home, &["start", "demo/web/nope"]);
    assert_eq!(code, 2);
    assert!(err.contains("no command or group named \"web/nope\""), "{err}");
}

#[test]
fn add_command_writes_config_or_the_groups_fragment() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let projects = home.join("Projects");
    let repo = home.join("api");
    std::fs::create_dir_all(repo.join(".switchboard")).unwrap();
    std::fs::write(repo.join(".switchboard/commands.yaml"), "commands:\n  - { id: dev, run: make dev }\n").unwrap();
    std::fs::create_dir_all(projects.join("hero")).unwrap();
    std::fs::write(
        projects.join("hero/config.yaml"),
        format!("version: 1\nname: Hero\ngroups:\n  - id: web\n    cwd: /tmp\n    commands:\n      - id: dev\n        run: npm run dev\n  - id: api\n    from: {}\n", repo.display()),
    )
    .unwrap();
    let pd = projects.to_str().unwrap();

    // Into an existing inline group.
    let (code, out, err) = sb(home, &["--projects-dir", pd, "add-command", "hero/web", "--id", "storybook", "--run", "npm run storybook", "--no-autostart", "--env", "PORT=6006"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("added hero/web/storybook"), "{out}");
    let cfg = std::fs::read_to_string(projects.join("hero/config.yaml")).unwrap();
    assert!(cfg.contains("id: storybook") && cfg.contains("autostart: false") && cfg.contains("PORT:") && cfg.contains("6006"), "{cfg}");

    // A group from a repo: the command lands in that repo's commands.yaml.
    let (code, _, err) = sb(home, &["--projects-dir", pd, "add-command", "hero/api", "--id", "worker", "--run", "make worker"]);
    assert_eq!(code, 0, "{err}");
    let frag = std::fs::read_to_string(repo.join(".switchboard/commands.yaml")).unwrap();
    assert!(frag.contains("id: worker"), "{frag}");

    // A new group needs its details; with them it is created.
    let (code, _, err) = sb(home, &["--projects-dir", pd, "add-command", "hero/admin", "--id", "dev", "--run", "yarn dev"]);
    assert_eq!(code, 2);
    assert!(err.contains("--group-name"), "{err}");
    let (code, _, err) = sb(home, &["--projects-dir", pd, "add-command", "hero/admin", "--id", "dev", "--run", "yarn dev", "--group-name", "Admin", "--group-cwd", "/tmp"]);
    assert_eq!(code, 0, "{err}");

    // Duplicates are refused.
    let (code, _, err) = sb(home, &["--projects-dir", pd, "add-command", "hero/web", "--id", "dev", "--run", "x"]);
    assert_eq!(code, 2);
    assert!(err.contains("already has a command \"dev\""), "{err}");

    let (_, out, _) = sb(home, &["--projects-dir", pd, "list"]);
    for addr in ["web/storybook", "api/worker", "admin/dev"] {
        assert!(out.contains(addr), "{addr} missing from:\n{out}");
    }
}
