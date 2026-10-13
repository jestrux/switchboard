---
id: sidebar
number: 6
title: Sidebar
category: features
description: How the list of groups and commands behaves, and what it remembers.
---

# Sidebar

## Collapsed groups

- **What:** Groups start collapsed and show a count, such as "4 idle" or "2 running".
  Selecting a command anywhere opens its group, so the selected row is never hidden.
- **Reach it:** Click a group row to open or close it.
- **State:** Open groups are remembered per Mac in the app's preferences, under
  `expandedGroups`. This is not written to the YAML.
- **Limits:** The state is shared by the board and the project's own window.

## Truncated names

- **What:** Command names, group labels and project titles stay on one line and end in an
  ellipsis when the sidebar is too narrow. Status text, the port and the apply badge keep
  their full width.
- **Reach it:** Hover a row to see the full name in its tooltip.
- **State:** None.
- **Limits:** None known.

## Resizable width

- **What:** The sidebar can be dragged wider or narrower, between 200 and 640 points.
- **Reach it:** Drag the edge between the sidebar and the pane.
- **State:** Saved per window in the app's preferences. A project window with no width of
  its own follows the board's width, and one that has its own keeps it.
- **Limits:** The saved width of a deleted project is left behind in the preferences.

## Hover actions

- **What:** Hovering a command row shows Start, or Stop and Restart when it is running.
  Hovering a group or project row shows Start all, Stop all and Restart all.
- **Reach it:** Hover. Every action is also in a context menu and in the menu bar.
- **State:** None.
- **Limits:** None known.

## Drag to reorder

- **What:** Projects, groups and commands can be dragged into a new order.
- **Reach it:** Drag a rail tile, a group row or a command row onto the slot it should take.
- **State:** Group and command order is written to the config. Project order is per Mac.
- **Limits:** A command cannot be dragged into a different group. Rewriting the config
  from the app drops YAML comments.

## Repo notices

- **What:** Notes above the sidebar's foot offer to keep a project's settings in a repo, or
  say that the repo's copy changed and offer Apply.
- **Reach it:** They appear when relevant. See [Config and repo files](09-config-and-repo-files.md).
- **State:** A dismissed offer is remembered per project on this Mac.
- **Limits:** None known.

## Foot

- **What:** The bottom of the sidebar shows how many commands are running, with Stop all
  when any are. On the board it also says how many projects are in their own windows.
- **Reach it:** Always visible.
- **State:** None.
- **Limits:** None known.
