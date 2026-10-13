use sb_core::config::{expand_tilde, ConfigError, ProjectConfig};
use sb_core::project::{self, Conflict, InstallError};
use std::path::PathBuf;

fn sample_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/kinpay")
}

#[test]
fn parses_kinpay_sample() {
    let cfg = ProjectConfig::load(&sample_dir().join("config.yaml")).expect("valid sample");
    assert_eq!(cfg.version, 1);
    assert_eq!(cfg.name, "Kinpay");
    assert_eq!(cfg.icon.as_deref(), Some("icon.png"));
    assert!(cfg.commands.is_empty());

    let group_ids: Vec<&str> = cfg.groups.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(group_ids, ["frontend", "backend", "tunnels"]);

    let cmds = cfg.resolve_commands();
    assert_eq!(cmds.len(), 9);
    let addresses: Vec<&str> = cmds.iter().map(|c| c.address.as_str()).collect();
    assert_eq!(
        addresses,
        [
            "frontend/dev-app",
            "frontend/dev-dashboard",
            "frontend/storybook",
            "frontend/studio",
            "backend/dev",
            "backend/migrate",
            "backend/flush-loans",
            "tunnels/tunnel",
            "tunnels/ngrok",
        ]
    );

    let home = dirs::home_dir().unwrap();
    let by = |a: &str| cmds.iter().find(|c| c.address == a).unwrap().clone();

    // Group default folder.
    let dev_app = by("frontend/dev-app");
    assert_eq!(dev_app.cwd, Some(home.join("Documents/web/kinpay")));
    assert_eq!(dev_app.display_name, "Frontend / dev:app");
    assert!(dev_app.autostart);
    assert!(!dev_app.restart_on_crash);

    // Command override of the group folder.
    let storybook = by("frontend/storybook");
    assert_eq!(storybook.cwd, Some(home.join("Documents/web/kinpay/app")));
    assert!(!storybook.autostart);

    let studio = by("frontend/studio");
    assert_eq!(studio.cwd, Some(home.join("Documents/web/kinpay/frame-studio-simple")));

    let dev = by("backend/dev");
    assert_eq!(dev.cwd, Some(home.join("Documents/web/kinpay-backend")));
    assert!(dev.autostart);
    assert!(dev.restart_on_crash);
    assert_eq!(dev.display_name, "Backend / make:dev");

    let migrate = by("backend/migrate");
    assert!(!migrate.autostart);
    assert_eq!(migrate.run, "make migrate");

    let ngrok = by("tunnels/ngrok");
    assert_eq!(ngrok.cwd, Some(home.join("Documents/web/kinpay")));
    assert_eq!(ngrok.display_name, "Tunnels / Ngrok");

    let autostart: Vec<&str> = cmds.iter().filter(|c| c.autostart).map(|c| c.address.as_str()).collect();
    assert_eq!(autostart, ["frontend/dev-app", "frontend/dev-dashboard", "backend/dev"]);
}

#[test]
fn defaults_apply_when_keys_missing() {
    let cfg = ProjectConfig::from_yaml(
        r#"
version: 1
name: Demo
commands:
  - id: a
    run: echo a
groups:
  - id: g
    commands:
      - id: b
        run: echo b
        autostart: false
"#,
    )
    .unwrap();
    cfg.validate().unwrap();
    let cmds = cfg.resolve_commands();
    assert_eq!(cmds[0].address, "a");
    assert_eq!(cmds[0].display_name, "a"); // name defaults to id, no prefix
    assert!(cmds[0].autostart);
    assert!(!cmds[0].restart_on_crash);
    assert_eq!(cmds[0].cwd, None); // neither set: not an error
    assert_eq!(cmds[1].address, "g/b");
    assert_eq!(cmds[1].display_name, "g / b");
    assert!(!cmds[1].autostart);
}

#[test]
fn ungrouped_commands_come_before_groups() {
    let cfg = ProjectConfig::from_yaml(
        "version: 1\nname: X\ngroups:\n  - id: g\n    commands:\n      - {id: b, run: b}\ncommands:\n  - {id: a, run: a}\n",
    )
    .unwrap();
    let addrs: Vec<String> = cfg.resolve_commands().into_iter().map(|c| c.address).collect();
    assert_eq!(addrs, ["a", "g/b"]);
}

#[test]
fn unknown_keys_survive_a_round_trip() {
    let text = r#"
version: 1
name: Demo
future_field: keep me
groups:
  - id: g
    name: G
    cwd: ~/x
    experimental: true
    commands:
      - id: a
        run: echo a
        env:
          PORT: "3000"
        health_check: http://localhost:3000
"#;
    let cfg = ProjectConfig::from_yaml(text).unwrap();
    assert_eq!(
        cfg.extra.get("future_field").and_then(|v| v.as_str()),
        Some("keep me")
    );
    assert_eq!(cfg.groups[0].extra.get("experimental").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        cfg.groups[0].commands[0].extra.get("health_check").and_then(|v| v.as_str()),
        Some("http://localhost:3000")
    );

    let yaml = cfg.to_yaml().unwrap();
    let again = ProjectConfig::from_yaml(&yaml).unwrap();
    assert_eq!(cfg, again);
    assert!(yaml.contains("future_field: keep me"));
    assert!(yaml.contains("health_check: http://localhost:3000"));
    // Unset booleans stay unset rather than being written as defaults.
    assert!(!yaml.contains("autostart"));
    assert!(!yaml.contains("restart_on_crash"));
}

#[test]
fn validation_reports_every_problem() {
    let cfg = ProjectConfig::from_yaml(
        r#"
version: 2
name: ""
color: red
commands:
  - id: dup
    run: x
  - id: dup
    run: ""
groups:
  - id: g
    commands:
      - id: a/b
        run: x
  - id: g
    commands: []
"#,
    )
    .unwrap();
    let Err(ConfigError::Invalid(errors)) = cfg.validate() else { panic!("expected errors") };
    let joined = errors.join("\n");
    assert!(joined.contains("version must be 1"), "{joined}");
    assert!(joined.contains("name must not be empty"), "{joined}");
    assert!(joined.contains("color must look like"), "{joined}");
    assert!(joined.contains("duplicate command id \"dup\""), "{joined}");
    assert!(joined.contains("has an empty run"), "{joined}");
    assert!(joined.contains("must not contain '/'"), "{joined}");
    assert!(joined.contains("duplicate group id \"g\""), "{joined}");
}

#[test]
fn bad_yaml_is_a_parse_error_not_a_panic() {
    let err = ProjectConfig::from_yaml("version: 1\nname: [unclosed").unwrap_err();
    assert!(matches!(err, ConfigError::Parse(_)));
}

#[test]
fn tilde_expansion() {
    let home = dirs::home_dir().unwrap();
    assert_eq!(expand_tilde("~"), home);
    assert_eq!(expand_tilde("~/a/b"), home.join("a/b"));
    assert_eq!(expand_tilde("/tmp/x"), PathBuf::from("/tmp/x"));
    assert_eq!(sb_core::config::collapse_tilde(&home.join("a")), "~/a");
}

#[test]
fn slugify_names() {
    assert_eq!(project::slugify("Kinpay"), "kinpay");
    assert_eq!(project::slugify("  My Cool  App! "), "my-cool-app");
    assert_eq!(project::slugify("***"), "project");
}

#[test]
fn install_validates_copies_and_handles_conflicts() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("Projects");

    // Invalid source: nothing copied.
    let bad = tmp.path().join("bad");
    std::fs::create_dir_all(&bad).unwrap();
    std::fs::write(bad.join("config.yaml"), "version: 1\nname: Bad\ncommands:\n  - id: a\n    run: ''\n").unwrap();
    let err = project::install_project(&bad, &projects, Conflict::Fail).unwrap_err();
    assert!(matches!(err, InstallError::Invalid(_)));
    assert!(!projects.join("bad").exists());

    // No config at all.
    let empty = tmp.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(matches!(
        project::install_project(&empty, &projects, Conflict::Fail).unwrap_err(),
        InstallError::NoConfig(_)
    ));

    // Valid sample installs under its slug with its icon.
    let p = project::install_project(&sample_dir(), &projects, Conflict::Fail).unwrap();
    assert_eq!(p.slug, "kinpay");
    assert!(p.config.is_ok());
    assert_eq!(p.icon_path, Some(projects.join("kinpay/icon.png")));
    assert!(projects.join("kinpay/config.yaml").is_file());
    assert!(!projects.join(".kinpay.installing").exists());

    // Conflict handling.
    assert!(matches!(
        project::install_project(&sample_dir(), &projects, Conflict::Fail).unwrap_err(),
        InstallError::Exists(_)
    ));
    let (slug, exists) = project::install_target(&sample_dir(), &projects).unwrap();
    assert_eq!((slug.as_str(), exists), ("kinpay", true));
    let both = project::install_project(&sample_dir(), &projects, Conflict::KeepBoth).unwrap();
    assert_eq!(both.slug, "kinpay-2");
    let replaced = project::install_project(&sample_dir(), &projects, Conflict::Replace).unwrap();
    assert_eq!(replaced.slug, "kinpay");

    let all = project::load_projects(&projects);
    let slugs: Vec<&str> = all.iter().map(|p| p.slug.as_str()).collect();
    assert_eq!(slugs, ["kinpay", "kinpay-2"]);
}

#[test]
fn broken_project_folder_still_loads_with_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("Projects/broken");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.yaml"), "name: [oops").unwrap();
    let projects = project::load_projects(tmp.path().join("Projects").as_path());
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name(), "broken");
    assert!(matches!(projects[0].config, Err(ConfigError::Parse(_))));
}
