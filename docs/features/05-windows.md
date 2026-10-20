---
id: windows
number: 5
title: All Projects and project windows
category: features
description: The main board, the project rail, and pulling a project into a window of its own.
---

# All Projects and project windows

## All Projects

- **What:** The main window. It lists every project that is not in a window of its own.
- **Reach it:** Project › Show All Projects (⌘0).
- **State:** The selected project and the pane layout are saved in the app's preferences.
- **Limits:** When every project has its own window, All Projects says so and shows nothing else.

## Project rail

- **What:** A column of project icons. The sidebar beside it lists one project at a time,
  with its running count and Start all in a header.
- **Reach it:** On by default. Settings › Sidebar › Project rail turns it off, which shows
  every project in one tree instead.
- **State:** A preference on this Mac.
- **Limits:** None known.

## A window per project

- **What:** A project can be pulled out of All Projects into its own window. It is then removed
  from All Projects, and the window shows only that project, with no project row.
- **Reach it:** Project › Open in New Window (⌘⇧N), or the button in the project header in All Projects.
- **State:** Open project windows are remembered and reopen at the next launch.
- **Limits:** One window per project. Opening it again focuses the existing window.

## Returning a project to All Projects

- **What:** Puts the project back in All Projects' list.
- **Reach it:** Project › Move to All Projects, or close the project's window.
- **State:** Removed from the list of windows to reopen.
- **Limits:** The project's commands keep running through the move.

## A look per window

- **What:** A project window can use its own theme and its own light or dark setting,
  whatever All Projects uses.
- **Reach it:** `theme:` and `appearance:` in the project's config. See [Themes](08-themes.md).
- **State:** In the config.
- **Limits:** The override applies only while the project is in its own window.
