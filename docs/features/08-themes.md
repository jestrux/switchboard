---
id: themes
number: 8
title: Themes
category: features
description: Built-in themes, light and dark, per-project themes and custom palettes.
---

# Themes

The [Theming guide](../theming.md) has the screenshots and the full token list. This page
is the summary.

## Built-in themes

- **What:** Three themes, Graphite, Patch Bay and Paper, each with a light and a dark
  palette that covers the chrome and the terminal's colours.
- **Reach it:** Settings (⌘,) › Theme. Changes apply at once and the preview shows the result.
- **State:** A preference on this Mac.
- **Limits:** None known.

## Light, dark, system

- **What:** The app follows macOS by default, or can be pinned to light or dark.
- **Reach it:** Settings › Appearance.
- **State:** A preference on this Mac.
- **Limits:** None known.

## Per-project themes

- **What:** A project can name a theme and an appearance for its own window.
- **Reach it:** `theme:` and `appearance:` in the project's config.
- **State:** In the config, so it travels with the project.
- **Limits:** It applies only while the project is in its own window, not in All Projects.

## Custom themes

- **What:** A project can define its own palette over a built-in base, with separate light
  and dark values. Kinpay, CityBuild, Thrive and Set Hero each have one.
- **Reach it:** A `theme:` mapping with `base`, `light` and `dark` in the project's config.
- **State:** In the config.
- **Limits:** Only the documented tokens are accepted. An unknown token is a validation error.
