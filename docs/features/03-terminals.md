---
id: terminals
number: 3
title: Terminals
category: features
description: Login shells opened on demand in a group's folder, kept per Mac.
---

# Terminals

## New terminal

- **What:** Opens a login shell in a group's folder, listed in that group beside its commands.
- **Reach it:** ⌘T, a group's hover button or context menu, or the picker in an empty pane.
- **State:** `terminals.yaml` in the project's folder in Application Support. It is working
  state for this Mac and is never written to a repo.
- **Limits:** After a relaunch a terminal comes back with a fresh shell. Its scrollback and
  running program are not restored.

## Rename, close, save as command

- **What:** A terminal can be renamed, closed, or turned into a real command with Save as Command….
- **Reach it:** Right-click the terminal's row. The hover close button also closes it.
- **State:** Save as Command writes to the group's `commands.yaml` when it has one,
  otherwise to `config.yaml`.
- **Limits:** Closing a terminal ends its shell at once.
