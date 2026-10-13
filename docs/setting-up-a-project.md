---
id: setting-up-a-project
number: 2
title: Setting up a project
category: guides
description: A checklist for taking a product folder to a themed, verified project.
---

# Setting up a project

How to turn "help me set up `<product>` at `<folder>`, including its theme" into a working
Switchboard project. Written for an agent, and it works as a checklist for a person.

The result is a project in `~/Library/Application Support/Switchboard/Projects/<slug>/`
with a `config.yaml`, an `icon.png`, and a custom theme that follows the system.

## Ground rules

- **The folder you are given is where the product lives. Commands belong to its app
  folders.** Do not add a command that runs at the product root unless asked. If the user
  names specific folders, use exactly those.
- **Install into Application Support only.** Do not write `.switchboard/` folders into the
  user's repos, and do not add the project to this repo's `samples/`, unless asked. The app
  offers "Keep settings in the repo?" and the user decides.
- **Never run commands that change data** while setting up or verifying: migrations, seeds,
  flushes, resets, deploys. Include useful ones with `autostart: false`; leave out the
  destructive ones (`flush`, `reset`, `drop`) and say so.
- Ask only if the folder gives no way to tell which sub-folders are the product. Otherwise
  proceed and report what you chose.

## 1. Start from Solo if there is a `solo.yml`

Check the folder and its immediate sub-folders:

```sh
sb import-solo <folder> --dry-run     # show the conversion
sb import-solo <folder>               # install it
```

Each Solo process becomes a command, grouped by working folder. `auto_start` and
`auto_restart` carry over. `restart_when_changed` has no equivalent and is reported.
Several folders can be passed to merge their Solo files into one project.

The conversion is a starting point, not the finished project. Continue with the steps
below on the installed `config.yaml`: check the commands run, tidy the names, add the theme.

## 2. Find the commands

For each app folder look at:

- `package.json` scripts, and which lockfile is there (npm, yarn, pnpm, bun)
- `Makefile` targets, `Procfile`, `composer.json` scripts, `docker-compose.yml`
- the README's "getting started", and any `start.sh` that shows how the pieces are run

Typical shape: a group per app folder, named for what it is (Frontend, Backend, E2E).

| Include | How |
|---|---|
| Dev servers | `autostart: true`; add `restart_on_crash: true` for backends |
| Storybook, workers, tunnels, docs sites | `autostart: false` |
| One-off tasks: typecheck, test, lint, migrate | `autostart: false` |
| Interactive runners (e2e pickers) | `autostart: false` |

## 3. Make sure each command runs from a login shell

Switchboard runs `$SHELL -lc "<run>"` in the command's folder. That is not the same as a
terminal you have been working in. Check before trusting a command:

```sh
cd <folder> && zsh -lc 'which <tool>; node -v'
```

Two problems have come up every time so far:

- **Python tools only in a virtualenv.** A Makefile that calls `uvicorn` or `celery` bare
  needs `run: source venv/bin/activate && make dev`. Check the venv folder's real name.
- **A pinned Node version.** A login shell does not switch on `.nvmrc`. Use
  `run: source ~/.nvm/nvm.sh && nvm use && <command>`. Old Grunt apps often need this,
  because `grunt` is installed under one specific Node version.

## 4. Icon

Use the product's own square icon: a 512 px favicon, an app icon, or a touch icon. Copy it
to `icon.png` in the project folder, resized to 512 × 512 (`sips -Z 512`). A wide logo
crops badly; look for a square mark first, and crop to the mark if there is none.

## 5. Theme

Find the product's design tokens. In these codebases they are CSS variables in the main
stylesheet (`app/app.css`, `src/index.css`), with a `:root` block for light and a `.dark`
block for dark. A marketing or landing page sometimes has a better palette than the app;
look at both and pick the one that looks best.

Write a custom theme with **both** `dark:` and `light:` maps and `appearance: system`, so
it follows macOS. Usual mapping from shadcn-style variables:

| Switchboard token | From |
|---|---|
| `window`, `rail` | the page canvas, or `--background` in dark |
| `sidebar` | `--sidebar`, else a step off the canvas |
| `pane`, `header` | `--card` |
| `terminal` | the darkest surface in dark, white or near-white in light |
| `text`, `text2`, `text3` | `--foreground`, a mid step, `--muted-foreground` |
| `line`, `buttonLine` | `--border` (soften it if it is a translucent white: hairlines get heavy) |
| `hover`, `selection` | `--muted`, `--accent` or a tint of the brand colour |
| `accent`, `onAccent` | the brand colour and the text colour used on it |
| `accentSoft`, `accentSoftText` | a low-contrast tint of the accent and a readable shade of it |
| `running`, `starting`, `crashed`, `error` | the product's success, warning and destructive colours, if it has them |
| `ansi` | optional; brand hues in the blue, magenta and green slots |

Check the accent is readable on the surface in each mode. A dark navy primary works in
light and disappears in dark, so the two modes often need different accents. Tokens you
leave out come from the `base` theme, so a small theme is fine. Convert `hsl()` values to
`#RRGGBB`. Put a one-line comment above the theme saying which file the colours came from.
Details and examples: [theming.md](theming.md).

## 6. Group icons

Groups get an icon from their name and commands automatically. Set `icon:` on a group only
when the automatic one is wrong. Keys: `frontend` `backend` `e2e` `terminal` `mobile`
`docs` `database` `worker` `tunnel` `infra` `ai` `design` `scripts` `storybook` `api`
`folder`.

## 7. Validate, install, verify

```sh
sb validate <folder with config.yaml>
sb install <folder with config.yaml>      # skip if you edited the installed config in place
sb run <slug>                             # boots the autostart commands; Ctrl+C after ~20 s
```

- Confirm each autostart command reaches a running state and prints what you expect.
- Confirm nothing is left on its ports afterwards.
- Open the project in its own window in the app (right-click › Open in New Window) and
  look at it in **both** dark and light. Temporarily set `appearance: dark` then `light` in
  the config to check each; put it back to `system`.

## 8. Report

Say what is in the project, where the theme came from, what you verified, and what you
left out and why. Call out anything broken in the product repo that you worked around or
did not fix (a wrong venv path in a script, expired credentials, ports already in use).

## Worked examples

| Project | Notes |
|---|---|
| [`samples/citybuild`](../samples/citybuild/config.yaml) | Three groups; theme from the landing page's ink, crimson and gold |
| [`samples/kinpay`](../samples/kinpay/config.yaml) | Six groups across three repos; purple theme from the app and dashboard |
| Thrive (installed, not in this repo) | Backend commands activate `venv`; navy and orange theme |
| Set Hero (installed, not in this repo) | Converted from `solo.yml`; `nvm use` for Node 6 and Node 22; neutral theme with a sky-blue accent in dark and navy in light |
