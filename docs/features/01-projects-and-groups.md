---
id: projects-and-groups
number: 1
title: Projects and groups
category: features
categoryTitle: Features
description: What a project is, how one is added and removed, and how groups organise its commands.
---

# Projects and groups

## Projects

- **What:** A project is a named set of commands across any folders. One product can span
  an API repo, a web repo and an e2e repo, with each command running in its own folder.
- **Reach it:** Each project is a tile in the rail, or a row in the tree when the rail is off.
- **State:** `~/Library/Application Support/Switchboard/Projects/<slug>/config.yaml`, with
  an optional `icon.png` beside it.
- **Limits:** There is no in-app editor. Edit Config (⌘E) opens the YAML in your editor.

## Adding a project

- **What:** Installs a project from a folder that holds a `config.yaml`, a
  `.switchboard/project.yaml` or a `solo.yml`.
- **Reach it:** File › Add Project… (⌘O), the plus button, or drop a folder on the sidebar
  or the Dock icon.
- **State:** The project is copied into the Projects directory. The source folder is not changed.
- **Limits:** A folder that is not a project shows a dialog and nothing is copied. A name
  that already exists offers Replace, Keep Both or Cancel.

## Removing a project

- **What:** Stops the project's commands and moves its folder in the Projects directory to the Trash.
- **Reach it:** Right-click the project, then Delete Project….
- **State:** The project's terminals go with it, because they live in the same folder.
- **Limits:** Sidebar width and pane layout saved for that project stay behind in the app's preferences.

## Groups

- **What:** A group holds commands that share a default folder, such as Frontend or
  Backend. Commands outside any group sit above the groups.
- **Reach it:** Declared under `groups:` in the config. A group row has Start all, Stop
  all, Restart all and New Terminal on hover and in its context menu.
- **State:** In the project's config.
- **Limits:** A group whose `from:` folder or fragment is missing is marked unavailable. The
  rest of the project still loads.

## Group icons

- **What:** Every group has an icon, chosen from its name and commands, with a folder as
  the fallback. The same icon shows in the pane bar.
- **Reach it:** Right-click a group, then Icon, to pick one or return to Automatic.
- **State:** `icon:` on the group in the config. Automatic icons are not written.
- **Limits:** The choices are a fixed set of sixteen keys, listed in
  [Projects, config and repo files](../projects.md).

## Project icons

- **What:** A project shows its `icon.png`, or its initials on the project's `color`.
- **Reach it:** Put a square `icon.png` beside the config.
- **State:** `icon:` and `color:` in the config.
- **Limits:** The app cannot import an icon. Copy the file in by hand.

## Broken configs

- **What:** A project whose config does not validate shows a card with every error instead
  of its commands.
- **Reach it:** The card has an Edit Config link. Saving a fixed file clears it within about a second.
- **State:** Nothing is changed on disk.
- **Limits:** A broken project cannot run anything until it validates.
