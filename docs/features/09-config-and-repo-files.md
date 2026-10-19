---
id: config-and-repo-files
number: 9
title: Config and repo files
category: features
description: Projects as YAML on disk, live reload, and keeping settings in your repos.
---

# Config and repo files

[Projects, config and repo files](../projects.md) has the format in full. This page is the summary.

## YAML on disk

- **What:** A project is a `config.yaml`. Unknown keys are kept, and an invalid file shows
  its errors in the sidebar.
- **Reach it:** Project › Edit Config (⌘E), Reveal in Finder, or Open Projects Folder.
- **State:** `~/Library/Application Support/Switchboard/Projects/<slug>/`.
- **Limits:** When the app rewrites the file, for a reorder or an icon, YAML comments are dropped.

## Live reload

- **What:** Saving a config updates the sidebar within about a second, without stopping anything.
- **Reach it:** Automatic. Project › Reload Projects forces it.
- **State:** None.
- **Limits:** A running command whose `run`, `cwd` or `env` changed needs a restart to apply.

## Settings in your repos

- **What:** A project's settings can live in a `.switchboard/` folder in its repos. One
  home folder holds `project.yaml`, and each member folder can hold a `commands.yaml`. A
  group with `from:` takes its commands from that folder, with per-command `overrides`.
- **Reach it:** Project › Keep Settings in Repo…, which lists the project's working folders
  and then Other Folder…. Existing projects get a one-time offer.
- **State:** The live config stays in Application Support. Every change is mirrored to the
  home folder's `project.yaml`.
- **Limits:** How two groups that share one folder map to fragments is undecided, so
  splitting inline commands into fragments is not built.

## Changes from a pull

- **What:** When the repo's `project.yaml` changes underneath, the sidebar shows "Settings
  changed in the repo" with Apply, and nothing is overwritten until you choose.
- **Reach it:** The notice above the sidebar's foot.
- **State:** None.
- **Limits:** None known.

## Setting up on a new Mac

- **What:** Dropping any member folder finds the `project.yaml`, in that folder or in a
  sibling repo that lists it, shows which folders resolved, and installs the project.
- **Reach it:** Drop the folder on the sidebar or the Dock icon, or run `switchboard import <folder>`.
- **State:** Installed into the Projects directory.
- **Limits:** When several copies exist the newest is proposed and the others can be chosen.
