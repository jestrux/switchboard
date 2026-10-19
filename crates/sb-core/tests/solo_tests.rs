//! Solo conversion, group icons, and reordering.

use sb_core::config::ProjectConfig;
use sb_core::engine::Engine;
use sb_core::icons::group_icon;
use sb_core::project::Conflict;
use sb_core::solo;
use std::path::Path;

/// Keep the engine's sessions.json out of the real app support folder.
fn isolate() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let dir = std::env::temp_dir().join(format!("sb-tests-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("SWITCHBOARD_HOME", dir);
    });
}

fn write(path: &Path, text: &str) {
    isolate();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[test]
fn solo_processes_become_commands_grouped_by_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("hero");
    for d in ["api", "web", "web/docs"] { std::fs::create_dir_all(root.join(d)).unwrap(); }
    write(&root.join("logo.png"), "png");
    write(&root.join("solo.yml"), &format!(
        "name: hero\nicon: logo.png\nprocesses:\n  api:\n    command: node src\n    working_dir: {r}/api\n    auto_start: true\n    auto_restart: true\n    restart_when_changed: [src]\n    env: {{PORT: \"4000\"}}\n  web:\n    command: npm run dev\n    working_dir: {r}/web\n    auto_start: false\n  web-storybook:\n    command: npm run storybook\n    working_dir: {r}/web\n  docs:\n    command: npm run dev\n    working_dir: {r}/web/docs\n",
        r = root.display()));
    let files = solo::find_solo_files(&root);
    assert_eq!(files.len(), 1);
    let import = solo::convert(&files).unwrap();
    let cfg = &import.config;
    assert_eq!(cfg.name, "hero");
    let groups: Vec<(&str, usize)> = cfg.groups.iter().map(|g| (g.id.as_str(), g.commands.len())).collect();
    assert_eq!(groups, [("api", 1), ("web", 2), ("docs", 1)], "grouped by folder, in file order");
    let api = &cfg.groups[0].commands[0];
    assert_eq!((api.run.as_str(), api.autostart, api.restart_on_crash), ("node src", Some(true), Some(true)));
    assert_eq!(api.env.get("PORT").map(String::as_str), Some("4000"));
    assert_eq!(cfg.groups[1].commands[0].autostart, Some(false));
    assert_eq!(cfg.groups[1].commands[1].name.as_deref(), Some("web-storybook"));
    assert_eq!(cfg.groups[1].commands[1].autostart, Some(true), "auto_start defaults to true");
    assert!(cfg.groups[2].cwd.as_deref().unwrap().ends_with("web/docs"));
    assert!(import.notes.iter().any(|n| n.contains("restart_when_changed")));
    assert!(import.icon.is_some());

    let projects = tmp.path().join("Projects");
    let dest = solo::install(&import, &projects, Conflict::Fail).unwrap();
    let loaded = ProjectConfig::load(&dest.join("config.yaml")).unwrap();
    assert_eq!(loaded.resolve_commands().len(), 4);
    assert!(dest.join("icon.png").is_file());

    // Files in subfolders are found and merged when the root has none.
    let multi = tmp.path().join("multi");
    write(&multi.join("a/solo.yml"), "name: A\nprocesses:\n  one:\n    command: echo 1\n");
    write(&multi.join("b/solo.yml"), "name: B\nprocesses:\n  two:\n    command: echo 2\n");
    let merged = solo::convert(&solo::find_solo_files(&multi)).unwrap();
    assert_eq!(merged.config.name, "A");
    assert_eq!(merged.config.groups.len(), 2);
}

#[test]
fn group_icons_are_explicit_or_derived_from_context() {
    assert_eq!(group_icon(Some("database"), "frontend", "Frontend", &[]), "database", "explicit wins");
    assert_eq!(group_icon(Some("nonsense"), "frontend", "Frontend", &[]), "frontend", "unknown falls back to derived");
    assert_eq!(group_icon(None, "frontend", "Frontend", &[]), "frontend");
    assert_eq!(group_icon(None, "circles-backend", "Circles Backend", &[]), "backend");
    assert_eq!(group_icon(None, "e2e", "E2E", &[]), "e2e");
    assert_eq!(group_icon(None, "tunnels", "Tunnels", &["ngrok http 3000"]), "tunnel");
    assert_eq!(group_icon(None, "hapi", "hapi", &["node src"]), "api");
    assert_eq!(group_icon(None, "new-cs", "new-cs", &["npm run dev", "npm run storybook"]), "storybook", "from what it runs");
    assert_eq!(group_icon(None, "wrapper", "Wrapper", &["./go"]), "folder", "no partial-word matches, clean fallback");
    let bad = ProjectConfig::from_yaml("version: 1\nname: X\ngroups:\n  - id: g\n    icon: sparkle\n").unwrap();
    assert!(bad.validate().unwrap_err().to_string().contains("icon must be one of"));
}

#[test]
fn groups_and_commands_can_be_reordered_and_icons_set() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("Projects");
    let repo = tmp.path().join("repo");
    write(&repo.join(".switchboard/commands.yaml"), "commands:\n  - {id: x, run: echo x}\n  - {id: y, run: echo y}\n  - {id: z, run: echo z}\n");
    write(&projects.join("demo/config.yaml"), &format!(
        "version: 1\nname: Demo\ncommands:\n  - {{id: u1, run: echo}}\n  - {{id: u2, run: echo}}\ngroups:\n  - id: a\n    commands:\n      - {{id: one, run: echo}}\n      - {{id: two, run: echo}}\n      - {{id: three, run: echo}}\n  - id: b\n    from: {}\n  - id: c\n    commands: []\n", repo.display()));
    let engine = Engine::new(&projects);
    let order = |e: &Engine| -> Vec<String> { e.project("demo").unwrap().effective.unwrap().resolve_commands().into_iter().map(|c| c.address).collect() };

    engine.move_group("demo", "c", 0).unwrap();
    engine.move_command("demo", "a/three", 0).unwrap();
    engine.move_command("demo", "u2", 0).unwrap();
    engine.move_command("demo", "b/z", 0).unwrap();   // fragment-backed
    assert_eq!(order(&engine), ["u2", "u1", "a/three", "a/one", "a/two", "b/z", "b/x", "b/y"]);
    let groups: Vec<String> = engine.project("demo").unwrap().config.unwrap().groups.iter().map(|g| g.id.clone()).collect();
    assert_eq!(groups, ["c", "a", "b"]);
    assert!(std::fs::read_to_string(repo.join(".switchboard/commands.yaml")).unwrap().find("id: z").unwrap()
        < std::fs::read_to_string(repo.join(".switchboard/commands.yaml")).unwrap().find("id: x").unwrap());
    assert!(engine.move_command("demo", "a/nope", 0).is_err());

    engine.set_group_icon("demo", "a", Some("database")).unwrap();
    assert_eq!(engine.project("demo").unwrap().config.unwrap().group("a").unwrap().icon.as_deref(), Some("database"));
    assert!(engine.set_group_icon("demo", "a", Some("bogus")).is_err());
    engine.set_group_icon("demo", "a", None).unwrap();
    assert_eq!(engine.project("demo").unwrap().config.unwrap().group("a").unwrap().icon, None);
}
