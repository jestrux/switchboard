---
id: panes
number: 4
title: Panes
category: features
description: The terminal pane, its bar and footer, and splitting a window into up to four panes.
---

# Panes

## The terminal pane

- **What:** A real terminal for the selected command, with colour and full interactivity,
  so programs such as vim, htop and Claude Code work. Hidden commands keep running and keep
  their scrollback.
- **Reach it:** Select a command in the sidebar, or pick one from the pane bar's menu.
- **State:** One terminal view per command for the life of the app.
- **Limits:** Scrollback is lost when the app quits. There is no find bar yet.

## Pane bar and footer

- **What:** The bar shows the status, the group's icon, the group and command name, the
  port when one is detected, and Start, Stop and Restart. The footer shows the folder, the
  process id and the uptime, or the exit summary.
- **Reach it:** Always visible above and below the terminal.
- **State:** None.
- **Limits:** Uptime ticks only in the footer. Sidebar rows do not count up.

## Never-run card

- **What:** A command that has not run yet shows its `run` line, its folder, its autostart
  setting and a Start button in place of an empty terminal.
- **Reach it:** Select a command that has never been started.
- **State:** None.
- **Limits:** None known.

## Clear

- **What:** Clears the focused terminal's screen and scrollback.
- **Reach it:** Command › Clear Terminal (⌘K).
- **State:** None.
- **Limits:** None known.

## Split panes

- **What:** A window can show up to four terminals at once, split right or down, with
  draggable dividers.
- **Reach it:** View › Split Right (⌘D) and Split Down (⌘⇧D), the pane bar's split button,
  or drag a command from the sidebar onto a pane edge. Dropping on the centre replaces the
  pane's command.
- **State:** The layout and the focused pane are saved per window in the app's preferences.
- **Limits:** Four panes at most. A command shows in one pane at a time.

## Focus, maximize, close

- **What:** Move between panes, blow one up to fill the window, or close it.
- **Reach it:** View › Next Pane (⌘]) and Previous Pane (⌘[), Maximize Pane (⌘⇧↩), Close
  Pane (⌘⇧W) and Reset Layout.
- **State:** Saved with the layout.
- **Limits:** Closing a pane does not stop its command. There is no ⌘⌥-arrow focus yet.

## Pane numbers in the sidebar

- **What:** When a window is split, each command shown in a pane carries that pane's number
  on its sidebar row.
- **Reach it:** Automatic once there are two or more panes.
- **State:** None.
- **Limits:** Hidden while the row is hovered, to make room for its buttons.
