//! Platform and machine overlays (`project.<os>.yaml`, `project.local.yaml`).

use sb_core::config::ProjectConfig;
use sb_core::fragment::ProjectSeed;
use sb_core::overlay::{platform_file, Overlay, LOCAL_FILE};
use std::path::Path;

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

const SEED: &str = "\
version: 1
name: Acme
commands:
  - id: claude
    run: claude
    cwd: ~/Documents/web/acme
    autostart: false
groups:
  - id: web
    cwd: ../web
    commands:
      - { id: dev, run: 'source ~/.nvm/nvm.sh && npm run dev', env: { PORT: '3000' } }
      - { id: test, run: npm test, autostart: false }
  - id: mobile
    cwd: ../mobile
    commands:
      - { id: metro, run: bun expo start }
      - { id: ios, run: bun expo run:ios, autostart: false }
  - id: api
    from: ../api
";

/// `acme/e2e` holds the seed; `acme/web`, `acme/mobile`, `acme/api` are siblings.
fn workspace(root: &Path) -> std::path::PathBuf {
    let home = root.join("acme/e2e");
    write(&home.join(".switchboard/project.yaml"), SEED);
    write(&root.join("acme/api/.switchboard/commands.yaml"), "name: API\ncommands:\n  - { id: dev, run: make dev }\n  - { id: seed, run: make seed, autostart: false }\n");
    std::fs::create_dir_all(root.join("acme/web")).unwrap();
    std::fs::create_dir_all(root.join("acme/mobile")).unwrap();
    home
}

fn composed(home: &Path) -> sb_core::fragment::Composed {
    ProjectSeed::load(home).unwrap().to_config(home).compose(home)
}

fn cmd<'a>(cfg: &'a ProjectConfig, group: Option<&str>, id: &str) -> Option<&'a sb_core::CommandConfig> {
    match group {
        Some(g) => cfg.group(g)?.commands.iter().find(|c| c.id == id),
        None => cfg.commands.iter().find(|c| c.id == id),
    }
}

#[test]
fn no_overlay_leaves_the_config_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let home = workspace(tmp.path());
    let c = composed(&home);
    assert!(c.overlays.is_empty());
    assert!(c.problems.is_empty(), "{:?}", c.problems);
    assert_eq!(cmd(&c.config, Some("web"), "dev").unwrap().run, "source ~/.nvm/nvm.sh && npm run dev");
    assert!(c.sources.iter().any(|s| s.ends_with(platform_file())), "overlay paths are watched even before they exist");
}

#[test]
fn platform_overlay_patches_hides_and_adds() {
    let tmp = tempfile::tempdir().unwrap();
    let home = workspace(tmp.path());
    write(
        &home.join(".switchboard").join(platform_file()),
        "\
commands:
  claude: { cwd: .. }
groups:
  web:
    commands:
      dev: { run: npm run dev, env: { HOST: 0.0.0.0 } }
      lint: { run: npm run lint, autostart: false }
      ghost: { autostart: true }
  mobile:
    commands:
      ios: { hidden: true }
  api:
    commands:
      seed: { hidden: true }
      dev: { restart_on_crash: true }
",
    );
    let c = composed(&home);
    assert_eq!(c.overlays.len(), 1);
    assert!(c.problems.is_empty(), "{:?}", c.problems);
    let cfg = &c.config;

    let claude = cmd(cfg, None, "claude").unwrap();
    assert!(claude.cwd.as_deref().unwrap().ends_with("acme"), "relative cwd resolves from the home folder: {:?}", claude.cwd);
    assert_eq!(claude.autostart, Some(false), "fields the overlay leaves out are kept");

    let dev = cmd(cfg, Some("web"), "dev").unwrap();
    assert_eq!(dev.run, "npm run dev");
    assert_eq!(dev.env.get("PORT").map(String::as_str), Some("3000"), "env is merged, not replaced");
    assert_eq!(dev.env.get("HOST").map(String::as_str), Some("0.0.0.0"));

    let web: Vec<&str> = cfg.group("web").unwrap().commands.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(web, ["dev", "test", "lint"], "a patch with run adds; one without run for an unknown id is ignored");

    assert!(cmd(cfg, Some("mobile"), "ios").is_none());
    assert!(cmd(cfg, Some("mobile"), "metro").is_some());

    // Fragment commands from `from:` groups can be patched too.
    assert!(cmd(cfg, Some("api"), "seed").is_none());
    assert_eq!(cmd(cfg, Some("api"), "dev").unwrap().restart_on_crash, Some(true));
}

#[test]
fn local_overlay_wins_over_platform_overlay() {
    let tmp = tempfile::tempdir().unwrap();
    let home = workspace(tmp.path());
    write(&home.join(".switchboard").join(platform_file()), "groups:\n  web:\n    commands:\n      dev: { run: npm run dev, autostart: false }\n");
    write(&home.join(".switchboard").join(LOCAL_FILE), "groups:\n  web:\n    commands:\n      dev: { run: npm run dev -- --port 4000 }\n");
    let c = composed(&home);
    assert_eq!(c.overlays.len(), 2);
    assert!(c.overlays[1].ends_with(LOCAL_FILE), "local applies last");
    let dev = cmd(&c.config, Some("web"), "dev").unwrap();
    assert_eq!(dev.run, "npm run dev -- --port 4000");
    assert_eq!(dev.autostart, Some(false), "platform fields the local overlay doesn't touch survive");
}

#[test]
fn group_overlay_moves_hides_and_creates_groups() {
    let tmp = tempfile::tempdir().unwrap();
    let home = workspace(tmp.path());
    std::fs::create_dir_all(tmp.path().join("elsewhere/web")).unwrap();
    write(
        &home.join(".switchboard").join(platform_file()),
        "\
groups:
  web: { cwd: ../../elsewhere/web, name: Website }
  mobile: { hidden: true }
  tunnels:
    icon: tunnel
    cwd: ..
    commands:
      ngrok: { run: ngrok http 3000, autostart: false }
",
    );
    let cfg = composed(&home).config;
    let web = cfg.group("web").unwrap();
    assert_eq!(web.name.as_deref(), Some("Website"));
    assert!(web.cwd.as_deref().unwrap().ends_with("elsewhere/web"), "{:?}", web.cwd);
    assert!(cfg.group("mobile").is_none());
    let tunnels = cfg.group("tunnels").unwrap();
    assert_eq!(tunnels.icon.as_deref(), Some("tunnel"));
    assert_eq!(tunnels.commands[0].run, "ngrok http 3000");
    cfg.validate().unwrap();
    let addrs: Vec<String> = cfg.resolve_commands().into_iter().map(|c| c.address).collect();
    assert!(addrs.contains(&"tunnels/ngrok".to_string()));
}

#[test]
fn group_overlay_can_repoint_a_from_group() {
    let tmp = tempfile::tempdir().unwrap();
    let home = workspace(tmp.path());
    write(&tmp.path().join("acme/api-v2/.switchboard/commands.yaml"), "commands:\n  - { id: serve, run: make serve }\n");
    write(&home.join(".switchboard").join(platform_file()), "groups:\n  api: { from: ../api-v2 }\n");
    let cfg = composed(&home).config;
    let ids: Vec<&str> = cfg.group("api").unwrap().commands.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["serve"]);
}

#[test]
fn broken_overlay_is_a_problem_not_a_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let home = workspace(tmp.path());
    write(&home.join(".switchboard").join(platform_file()), "groups:\n  web:\n    comands: {}\n");
    let c = composed(&home);
    assert!(c.overlays.is_empty());
    assert_eq!(c.problems.len(), 1);
    assert!(c.problems[0].message.contains("comands"), "typos are caught: {}", c.problems[0].message);
    assert_eq!(cmd(&c.config, Some("web"), "dev").unwrap().run, "source ~/.nvm/nvm.sh && npm run dev");
}

#[test]
fn overlays_never_reach_the_written_seed() {
    let tmp = tempfile::tempdir().unwrap();
    let home = workspace(tmp.path());
    write(&home.join(".switchboard").join(platform_file()), "groups:\n  web:\n    commands:\n      dev: { run: npm run dev }\n");
    let seed = ProjectSeed::load(&home).unwrap();
    let cfg = seed.to_config(&home);
    assert_eq!(cmd(&cfg.compose(&home).config, Some("web"), "dev").unwrap().run, "npm run dev");
    // Write-back is built from the raw config, which the overlay never touched.
    let back = ProjectSeed::from_config(&cfg, &home);
    assert!(back.same_settings(&seed, &home));
    assert!(back.config.to_yaml().unwrap().contains("source ~/.nvm/nvm.sh"));
}

#[test]
fn overlay_parses_minimal_and_empty_files() {
    assert_eq!(Overlay::from_yaml("{}").unwrap(), Overlay::default());
    assert!(Overlay::from_yaml("groups:\n  web:\n    hidden: true\n").unwrap().groups["web"].hidden);
}
