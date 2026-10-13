---
id: projects-config
number: 1
title: Projects, config and repo files
category: guides
categoryTitle: Guides
description: The config format, repo files, terminals and the sb CLI.
---

# Projects, config and repo files

- [The config file](#the-config-file)
- [Versioning a project with its repos](#versioning-a-project-with-its-repos-switchboard)
- [Terminals](#terminals)
- [The `sb` CLI](#the-sb-cli)

## The config file

A project is a named group of commands across any folders. It lives in
`~/Library/Application Support/Switchboard/Projects/<slug>/config.yaml`, next to an
optional `icon.png`.

```yaml
version: 1
name: Kinpay
icon: icon.png              # optional; falls back to initials on `color`
color: "#9B3FE6"

commands: []                # ungrouped commands, shown above the groups

groups:
  - id: frontend
    name: Frontend
    cwd: ~/Documents/web/kinpay       # default folder for the group's commands
    commands:
      - id: dev-app                   # address: frontend/dev-app
        name: dev:app
        run: yarn dev:app
        autostart: true               # default true
      - id: storybook
        cwd: ~/Documents/web/kinpay/app   # overrides the group folder
        run: npm run storybook
        autostart: false
        restart_on_crash: false           # default false
        env: { PORT: "6006" }
```

- Commands run through your login shell (`$SHELL -lc`) in a real terminal, each in its own
  process group. Stop sends SIGINT, then SIGTERM after 3 s, then SIGKILL after 3 more.
- `restart_on_crash` retries with backoff (1, 2, 4 … 30 s) and gives up after 5 crashes in
  2 minutes.
- Saving the file updates the sidebar within about a second without stopping anything. A
  changed `run`, `cwd` or `env` shows "restart to apply".
- Unknown keys are kept. An invalid file shows its errors in the sidebar with Edit Config.
- `theme:` and `appearance:` are covered in [Theming](theming.md).
- **Order** in the file is the order in the sidebar. Dragging a group or a command in the
  app rewrites it. Project order in the rail is per Mac and is not stored in the config.
  Rewriting from the app keeps unknown keys but **drops YAML comments**.
- **Group icons**: a group gets an icon from its name and commands. Set `icon:` to choose
  one (`frontend`, `backend`, `e2e`, `terminal`, `mobile`, `docs`, `database`, `worker`,
  `tunnel`, `infra`, `ai`, `design`, `scripts`, `storybook`, `api`, `folder`), or use the
  group's right-click › Icon menu. The icon also shows in the pane bar.

## Versioning a project with its repos (`.switchboard/`)

A project spans folders, so its settings are split the same way and composed.

```
~/code/acme-app/.switchboard/
  commands.yaml     # this folder's commands (cwd is the folder)
  project.yaml      # the project: name, icon, theme, and its member folders
  icon.png
~/code/acme-api/.switchboard/
  commands.yaml
```

```yaml
# acme-app/.switchboard/commands.yaml
name: Web
commands:
  - { id: dev, run: npm run dev }
  - { id: storybook, run: npm run storybook, autostart: false }
```

```yaml
# acme-app/.switchboard/project.yaml   (the "home" folder)
version: 1
name: Acme
groups:
  - id: web
    from: .                 # this folder's commands.yaml
  - id: api
    from: ../acme-api       # a sibling repo; relative to the home folder
    overrides:
      migrate: { hidden: true }
      dev: { autostart: true, env: { PORT: "8080" } }
```

- **A group with `from:`** takes its commands from that folder's `commands.yaml`. `overrides`
  tweak them per command (`name`, `autostart`, `restart_on_crash`, `env`, `hidden`) without
  touching the other repo. Inline `commands:` still work and are appended. A missing folder
  or fragment marks that one group unavailable; the rest of the project loads.
- **New device**: drop any member folder on the sidebar or the Dock icon (or
  `sb import <folder>`). Switchboard finds the `project.yaml`, in that folder or in a sibling
  repo that lists it, shows which folders resolved, and installs the project. If several
  copies exist, the newest is proposed and the others can be chosen.
- **Write-back**: the live config stays in Application Support. Once a project has a home
  folder, every change is mirrored to that folder's `project.yaml`, so git always has the
  current settings. If the repo's copy changes underneath (a pull), the sidebar shows
  "Settings changed in the repo · Apply" instead of overwriting anything.
- **Existing projects** get a one-time offer to keep their settings in a repo folder
  (Project › Keep Settings in Repo… does it later).
- Fragments are watched: editing a repo's `commands.yaml` updates the sidebar live.
- `sb validate <repo folder>` checks a `project.yaml` and names every fragment it reads.

## Terminals

⌘T, a group's hover button or context menu, or the empty-pane picker opens a login shell in
that group's folder. Terminals are working state for this Mac: they are kept in the
project's `terminals.yaml` in Application Support, come back after a relaunch with a fresh
shell, and go away with the project. Right-click one to rename it, close it, or **Save as
Command…**, which adds a real command to its group (in the group's `commands.yaml` when it
has one, otherwise in `config.yaml`).

## The `sb` CLI

The same engine as the app, for the terminal.

```sh
sb list                         # projects, commands, live status
sb run kinpay                   # autostart commands, prefixed output; Ctrl+C stops all
sb run kinpay backend           # one group
sb run kinpay backend/migrate   # one command, interactive
sb run path/to/folder           # a project folder that is not installed
sb validate <folder|file>       # check a config.yaml or a repo's project.yaml
sb install <folder>             # copy a config.yaml project into the Projects directory
sb import <repo folder>         # set a project up from .switchboard/project.yaml
sb import-solo <folder>…        # convert Solo's solo.yml (--dry-run to preview)
sb cleanup                      # stop sessions left behind by a crash
```
