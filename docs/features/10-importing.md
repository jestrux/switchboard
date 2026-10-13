---
id: importing
number: 10
title: Importing
category: features
description: Bringing a project in from a folder, a repo or Solo.
---

# Importing

## From a project folder

- **What:** A folder with a `config.yaml` is installed as a project.
- **Reach it:** File › Add Project… (⌘O), drop it on the sidebar or the Dock icon, or run
  `sb install <folder>`.
- **State:** Copied into the Projects directory.
- **Limits:** An existing name asks for Replace, Keep Both or Cancel.

## From a repo

- **What:** A folder with `.switchboard/project.yaml`, or a member folder of one, sets the
  project up from the repo's settings.
- **Reach it:** The same drop, or `sb import <folder>`. See
  [Config and repo files](09-config-and-repo-files.md).
- **State:** Installed into the Projects directory, with the repo folder as its home.
- **Limits:** None known.

## From Solo

- **What:** Solo's `solo.yml` is converted. Every process becomes a command, and commands
  are grouped by their working folder.
- **Reach it:** Drop a folder that holds a `solo.yml`, or run `sb import-solo <folder>`.
  Add `--dry-run` to preview the result.
- **State:** Written as an ordinary `config.yaml`. The `solo.yml` is left alone.
- **Limits:** The in-app dialog for this was not click-tested. The CLI path was.

## Setting up a product by hand

- **What:** A checklist for taking a product's folders to a themed, verified project.
- **Reach it:** [Setting up a project](../setting-up-a-project.md).
- **State:** None.
- **Limits:** Commands belong to a product's app folders, not to its root folder.
