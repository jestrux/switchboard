---
id: features
number: 1
title: Feature list
category: overview
categoryTitle: Feature list
description: Everything Switchboard does today, by area, and what is not built yet.
---

# Feature list

This is the record of what Switchboard does. Each area below has its own page, and each
feature on a page says what it does, how to reach it, where its state lives and its limits.

Run `scripts/docs.sh` to browse these pages in a browser. The files in `docs/` are the
source, so reading them directly works just as well.

## Areas

| Area | What is in it |
|---|---|
| [Projects and groups](01-projects-and-groups.md) | What a project is, adding and removing one, groups, icons, broken configs |
| [Running commands](02-running-commands.md) | Start, stop, restart, statuses, crash restarts, restart to apply, quitting, starting again at launch |
| [Terminals](03-terminals.md) | Shells on demand in a group's folder, kept per Mac |
| [Panes](04-panes.md) | The terminal pane, its bar and footer, splits up to four |
| [Board and project windows](05-windows.md) | The main board, the rail, a window per project |
| [Sidebar](06-sidebar.md) | Collapsed groups, resizing, reordering, hover actions, truncated names |
| [Links and ports](07-links-and-ports.md) | ⌘-click links in output, image previews, the port a server is listening on |
| [Themes](08-themes.md) | Built-in themes, per-project themes, custom palettes |
| [Config and repo files](09-config-and-repo-files.md) | YAML on disk, live reload, `.switchboard/` in your repos |
| [Importing](10-importing.md) | Dropping a folder, converting Solo's `solo.yml` |
| [Settings and startup](11-settings-and-startup.md) | The Settings window, open at login, what is remembered per Mac |
| [The switchboard command](12-cli.md) | Drive the app from a terminal or an agent, the agent skill, and running without the app |

## Not built yet

- Editing sheets for projects, groups and commands. Today you edit the YAML.
- Dragging a command into a different group.
- A find bar for terminal output.
- Quick add from a folder's scripts.
- A menu bar extra.
- Project export.
- Splitting an existing project's inline commands into per-folder `commands.yaml` files.
- Icon import from the app.
- A per-project Dock icon for project windows. This was considered and declined.

`PLAN.md` at the repo root has the order and the context for the items that are planned.

## Keeping this list true

After meaningful feature work, an agent asks the author whether to update these pages, and
on a yes updates them in the same piece of work. Add a new feature under the area it
belongs to, with the same four points: what, reach it, state, limits. Remove an entry from
"Not built yet" when it ships.
