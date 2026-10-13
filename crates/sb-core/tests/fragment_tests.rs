//! Fragments, `from:` composition, project seeds, write-back and ad-hoc terminals.

use sb_core::config::ProjectConfig;
use sb_core::engine::{Engine, Status};
use sb_core::fragment::{discover_seeds, install_seed, Fragment, ProjectSeed};
use sb_core::project::Conflict;
use sb_core::session::CommandKey;
use std::path::Path;
use std::time::Duration;

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A workspace: `web/api` and `web/app` each with a fragment, `web/app` holding the seed.
fn workspace(root: &Path) {
    write(&root.join("web/api/.switchboard/commands.yaml"), "name: API\ncommands:\n  - id: dev\n    run: make dev\n  - id: migrate\n    run: make migrate\n    autostart: false\n    cwd: db\n");
    write(&root.join("web/app/.switchboard/commands.yaml"), "name: Web\ncommands:\n  - id: dev\n    run: npm run dev\n  - id: storybook\n    run: npm run storybook\n    autostart: false\n");
    std::fs::create_dir_all(root.join("web/api/db")).unwrap();
    write(&root.join("web/app/.switchboard/project.yaml"), "updated: 2026-10-13T10:00:00Z\nversion: 1\nname: Acme\ngroups:\n  - id: app\n    from: ../app\n    overrides:\n      storybook: { autostart: true }\n  - id: api\n    from: ../api\n    overrides:\n      migrate: { hidden: true }\n  - id: tools\n    from: ../tools\n");
}

#[test]
fn from_groups_compose_fragments_with_overrides() {
    let tmp = tempfile::tempdir().unwrap();
    workspace(tmp.path());
    let yaml = format!(
        "version: 1\nname: Acme\nhome: {}\ngroups:\n  - id: app\n    from: ../app\n    overrides:\n      storybook: {{ autostart: true, name: Stories }}\n    commands:\n      - id: extra\n        run: echo extra\n  - id: api\n    from: ../api\n    overrides:\n      migrate: {{ hidden: true }}\n  - id: tools\n    from: ../tools\n",
        tmp.path().join("web/app").display()
    );
    let cfg = ProjectConfig::from_yaml(&yaml).unwrap();
    cfg.validate().unwrap();
    let composed = cfg.compose(tmp.path());
    let c = &composed.config;
    let app = c.group("app").unwrap();
    assert_eq!(app.name.as_deref(), Some("Web"), "group name comes from the fragment");
    assert_eq!(app.cwd.as_deref().map(|s| s.ends_with("web/app")), Some(true));
    let ids: Vec<&str> = app.commands.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["dev", "storybook", "extra"], "fragment commands first, inline after");
    let sb = &app.commands[1];
    assert_eq!(sb.autostart, Some(true));
    assert_eq!(sb.name.as_deref(), Some("Stories"));
    let api = c.group("api").unwrap();
    assert_eq!(api.commands.len(), 1, "hidden override removes migrate");
    // A relative cwd inside a fragment resolves against the fragment folder.
    let cfg2 = ProjectConfig::from_yaml(&yaml.replace("migrate: { hidden: true }", "dev: {}")).unwrap();
    let api2 = cfg2.compose(tmp.path()).config.resolve("api/migrate").unwrap();
    assert_eq!(api2.cwd.unwrap(), tmp.path().join("web/api/db"));
    // Missing folder: group kept, problem recorded, nothing else breaks.
    assert_eq!(composed.problems.len(), 1);
    assert_eq!(composed.problems[0].group, "tools");
    assert!(composed.problems[0].message.contains("Folder not found"));
    assert!(c.group("tools").unwrap().commands.is_empty());
    assert_eq!(composed.sources.len(), 3);
}

#[test]
fn seeds_are_discovered_newest_first_and_install_with_resolved_paths() {
    let tmp = tempfile::tempdir().unwrap();
    workspace(tmp.path());
    // A drifted, older copy in the api repo.
    write(&tmp.path().join("web/api/.switchboard/project.yaml"), "updated: 2026-10-01T00:00:00Z\nversion: 1\nname: Acme Old\ngroups:\n  - id: app\n    from: ../app\n");
    let found = discover_seeds(&tmp.path().join("web/api"));
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].seed.config.name, "Acme", "newest first");
    assert!(found[0].home.ends_with("web/app"));
    let members: Vec<(String, bool)> = found[0].members.iter().map(|(g, _, e)| (g.clone(), *e)).collect();
    assert_eq!(members, [("app".into(), true), ("api".into(), true), ("tools".into(), false)]);

    let projects = tmp.path().join("Projects");
    let dest = install_seed(&found[0], &projects, Conflict::Fail).unwrap();
    assert!(dest.ends_with("acme"));
    let installed = ProjectConfig::load(&dest.join("config.yaml")).unwrap();
    assert_eq!(installed.home.as_deref().map(|h| h.ends_with("web/app")), Some(true));
    assert!(installed.groups[0].from.as_deref().unwrap().ends_with("web/app"));
    assert!(!installed.groups[0].from.as_deref().unwrap().starts_with(".."), "paths are absolute after install");

    // Round trip back to a seed: relative paths again, `home` dropped, stamped.
    let seed = ProjectSeed::from_config(&installed, &tmp.path().join("web/app"));
    assert_eq!(seed.config.groups[0].from.as_deref(), Some("."), "the home folder itself");
    assert_eq!(seed.config.groups[1].from.as_deref(), Some("../api"), "a sibling repo");
    assert!(seed.config.home.is_none());
    assert!(seed.updated.is_some());
    assert!(seed.same_settings(&found[0].seed, &found[0].home));
}

#[test]
fn engine_writes_seed_back_and_flags_repo_changes() {
    let tmp = tempfile::tempdir().unwrap();
    workspace(tmp.path());
    let projects = tmp.path().join("Projects");
    let found = discover_seeds(&tmp.path().join("web/app"));
    install_seed(&found[0], &projects, Conflict::Fail).unwrap();
    let engine = Engine::new(&projects);
    let p = engine.project("acme").unwrap();
    assert_eq!(p.problems.len(), 1);
    assert!(!p.repo_settings_changed);
    // The engine composed fragments: the app group's commands come from web/app.
    let eff = p.effective.as_ref().unwrap();
    assert_eq!(eff.resolve("app/storybook").unwrap().autostart, true);

    // Local edit to config.yaml → the seed in the repo follows.
    let cfg_path = projects.join("acme/config.yaml");
    let edited = std::fs::read_to_string(&cfg_path).unwrap().replace("name: Acme", "name: Acme Renamed");
    std::thread::sleep(Duration::from_millis(20));
    std::fs::write(&cfg_path, edited).unwrap();
    engine.reload();
    let seed = ProjectSeed::load(&tmp.path().join("web/app")).unwrap();
    assert_eq!(seed.config.name, "Acme Renamed");
    assert!(!engine.project("acme").unwrap().repo_settings_changed);

    // Repo edit (a pull) → flagged, not applied, until apply_repo_settings.
    let seed_path = ProjectSeed::path_in(&tmp.path().join("web/app"));
    let pulled = std::fs::read_to_string(&seed_path).unwrap().replace("Acme Renamed", "Acme From Git");
    std::fs::write(&seed_path, pulled).unwrap();
    engine.reload();
    let p = engine.project("acme").unwrap();
    assert!(p.repo_settings_changed);
    assert_eq!(p.name(), "Acme Renamed", "local config untouched until applied");
    engine.apply_repo_settings("acme").unwrap();
    let p = engine.project("acme").unwrap();
    assert_eq!(p.name(), "Acme From Git");
    assert!(!p.repo_settings_changed);
}

#[test]
fn adopting_a_home_writes_the_seed_and_fragments_promote_terminals() {
    let tmp = tempfile::tempdir().unwrap();
    workspace(tmp.path());
    let projects = tmp.path().join("Projects");
    // A plain app-local project with an inline group and a from: group, no home yet.
    write(&projects.join("demo/config.yaml"), &format!(
        "version: 1\nname: Demo\ngroups:\n  - id: web\n    from: {}\n  - id: misc\n    cwd: /tmp\n    commands:\n      - id: hi\n        run: echo hi\n",
        tmp.path().join("web/app").display()
    ));
    let engine = Engine::new(&projects);
    let home = tmp.path().join("web/app");
    engine.adopt_home("demo", &home).unwrap();
    let seed = ProjectSeed::load(&home).unwrap();
    assert_eq!(seed.config.name, "Demo");
    assert_eq!(seed.config.groups[0].from.as_deref(), Some("."));
    assert_eq!(engine.project("demo").unwrap().home(), Some(home.clone()));

    // Terminals: open one in each kind of group and ungrouped.
    let t_web = engine.add_terminal("demo", Some("web")).unwrap();
    let t_misc = engine.add_terminal("demo", Some("misc")).unwrap();
    let t_none = engine.add_terminal("demo", None).unwrap();
    assert!(t_web.starts_with("@terminal/"));
    let p = engine.project("demo").unwrap();
    assert_eq!(p.terminals.len(), 3);
    assert_eq!(p.terminals[1].name, "Terminal");
    let eff = p.effective.as_ref().unwrap();
    let web_t = eff.resolve(&t_web).unwrap();
    assert!(web_t.terminal);
    assert_eq!(web_t.display_name, "Web / Terminal");
    assert_eq!(web_t.cwd.unwrap(), home);
    assert_eq!(eff.resolve(&t_misc).unwrap().cwd.unwrap(), Path::new("/tmp"));
    // The shells are running interactive sessions.
    let key = CommandKey::new("demo", &t_misc);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while engine.state_of(&key).status != Status::Running && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(30));
    }
    assert_eq!(engine.state_of(&key).status, Status::Running);
    engine.write_input(&key, b"echo terminal-ok\n").unwrap();
    std::thread::sleep(Duration::from_millis(800));
    assert!(engine.screen_text(&key).contains("terminal-ok"));
    // terminals.yaml holds them; the project config does not.
    assert!(projects.join("demo/terminals.yaml").exists());
    assert!(!std::fs::read_to_string(projects.join("demo/config.yaml")).unwrap().contains("@terminal"));

    engine.rename_terminal("demo", &t_none, "scratch").unwrap();
    assert_eq!(engine.project("demo").unwrap().terminals[2].name, "scratch");

    // Promote: into the fragment for a from: group, into config.yaml for an inline group.
    let addr = engine.promote_terminal("demo", &t_web, "Lint", "lint", "npm run lint").unwrap();
    assert_eq!(addr, "web/lint");
    let fragment = Fragment::load(&home).unwrap();
    assert!(fragment.commands.iter().any(|c| c.id == "lint" && c.run == "npm run lint"));
    let addr2 = engine.promote_terminal("demo", &t_misc, "Bye", "bye", "echo bye").unwrap();
    assert_eq!(addr2, "misc/bye");
    assert!(std::fs::read_to_string(projects.join("demo/config.yaml")).unwrap().contains("echo bye"));
    let p = engine.project("demo").unwrap();
    assert_eq!(p.terminals.len(), 1);
    assert!(p.effective.as_ref().unwrap().resolve("web/lint").is_some());
    engine.remove_terminal("demo", &t_none).unwrap();
    assert!(!projects.join("demo/terminals.yaml").exists());
    engine.stop_everything_blocking();
}

#[test]
fn a_member_folder_without_a_seed_finds_its_home_next_door() {
    let tmp = tempfile::tempdir().unwrap();
    workspace(tmp.path());
    // web/api has a fragment but no project.yaml; web/app's seed lists it.
    let found = discover_seeds(&tmp.path().join("web/api"));
    assert_eq!(found.len(), 1);
    assert!(found[0].home.ends_with("web/app"));
    // An unrelated folder finds nothing.
    std::fs::create_dir_all(tmp.path().join("web/other")).unwrap();
    assert!(discover_seeds(&tmp.path().join("web/other")).is_empty());
}
