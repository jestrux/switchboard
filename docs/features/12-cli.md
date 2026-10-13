---
id: cli
number: 12
title: The sb CLI
category: features
description: The same engine as the app, from a terminal.
---

# The sb CLI

## Commands

- **What:** `sb` lists, runs, validates, installs and imports projects with the same engine
  the app uses.
- **Reach it:** Built by `cargo build -p sb-cli`. The commands are listed in
  [Projects, config and repo files](../projects.md).
- **State:** It reads and writes the same Projects directory as the app.
- **Limits:** None known.

## Running a project

- **What:** `sb run <project>` starts the autostart commands and prints their output with a
  prefix per command. A group or a single command can be named, and a single command runs
  interactively.
- **Reach it:** `sb run kinpay`, `sb run kinpay backend`, `sb run kinpay backend/migrate`.
- **State:** Sessions are recorded like the app's, so `sb list` shows live status.
- **Limits:** Ctrl+C stops everything that run started.

## Validate and clean up

- **What:** `sb validate` checks a `config.yaml` or a repo's `project.yaml` and names every
  fragment it reads. `sb cleanup` stops sessions left behind by a crash.
- **Reach it:** `sb validate <folder or file>` and `sb cleanup`.
- **State:** None.
- **Limits:** None known.
