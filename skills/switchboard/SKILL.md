---
name: switchboard
description: Run, watch and debug a project's dev servers in Switchboard (the Mac app, or its host and terminal window on Linux) with the `switchboard` command. Start, stop or restart commands, wait until a server is ready, read its output, and set a product up as a Switchboard project. Use when the user mentions Switchboard, or asks to start, run, restart or check a dev server or read its logs, and `switchboard` is installed.
---

# Switchboard

Switchboard runs a project's dev commands (servers, workers, watchers, one-off tasks) in
its own terminals. The author can see every one of them there. The `switchboard` command
lets you drive it. Commands you start this way run in Switchboard, not in your shell, so
they keep running after you finish. The author can watch them, and you can read their
output later.

It runs on two kinds of machine, and every command below works the same on both:

- **macOS:** the Switchboard app hosts the commands. Check with `command -v switchboard`;
  if it is missing, run `/Applications/Switchboard.app/Contents/Helpers/switchboard setup`,
  which links the command onto the PATH and this skill into `~/.claude/skills` (the app's
  **Switchboard › Install switchboard Command…**). If Switchboard.app is not in
  Applications, ask the author to install it. Do not build the command yourself.
- **Linux (Omarchy):** there is no app. `switchboard serve` hosts the commands in the
  background, and the first command that needs it (`start`, `wait`, …) launches it. The
  author watches in `switchboard ui`, a terminal window (the Omarchy menu's Switchboard
  entry). See [On Linux](#on-linux) for installing and what differs.

## Addressing

Projects have a slug, and commands have an address inside the project: `group/command`,
or just `command` when ungrouped. On the command line, join the two with a slash:

```
kinpay                    the whole project
kinpay/backend            one group
kinpay/backend/api        one command
```

`switchboard status` lists every slug and address. Read it first; never guess.

## The loop

```sh
switchboard status [<slug>]               # what exists and what runs: status, port, URL
switchboard start <slug>                  # the autostart commands (launches the app, or `serve`, if needed)
switchboard wait <slug>/<group>/<cmd>     # block until it prints its URL
switchboard logs <slug>/<group>/<cmd> -n 80
switchboard restart <slug>/<group>/<cmd>  # after changing its config or env
switchboard stop <slug>[/<group>[/<cmd>]] # returns once they have exited
```

- **`start` on a project or group** starts only its autostart commands. Naming a single
  command starts it whether or not it is autostart. Use that for one-off tasks such as
  tests, a typecheck or a build.
- **`wait`** checks one of three conditions:
  - `--for ready` (the default): the command printed the URL it listens on.
  - `--for running`: the process is up. Use this for workers that never print a URL.
  - `--for exit`: the command finished. It then exits with the command's own exit code.

  `wait` exits 1 when the command crashes or exits before the condition is met, and
  prints its last 40 lines. It exits 124 on `--timeout` (120 s by default).
- **For a one-off task**, run `switchboard start <slug>/<group>/test` and then
  `switchboard wait <slug>/<group>/test --for exit`. Read `logs` for the details.
- **`status --json`** gives machine-readable state per command: `status` (stopped,
  starting, running, exited, crashed, error), `pid`, `port`, `url`, `exit_code`,
  `message` and `needs_restart`.
- **`open <slug>`** brings the app forward on that project, for when the author wants to
  look. On Linux there is no window to raise; tell the author to open Switchboard from the
  Omarchy menu (or run `switchboard ui`).

## Rules

- **Read the logs before asking what broke.** When a command crashes or `wait` fails,
  read `switchboard logs` first. Report what it says, then fix it or ask.
- **Stop and start through `switchboard`**, never with `kill`, `pkill` or Ctrl+C on the
  app's processes. The app tracks them, and a process killed behind its back shows as
  crashed and may be restarted.
- **Do not run commands that change data unless the author asked:** migrations, seeds,
  flushes, resets, deploys. They are often configured with `autostart: false` for exactly
  this reason.
- **"Port in use" is often not a bug.** A server the author started outside Switchboard
  may already hold the port. Say so instead of killing it.
- **Prefer the app to `switchboard run`.** `run` starts commands in your own terminal and
  stops them when it exits, so the author cannot see them in the app. Use it only when
  the app is not an option.
- A command marked `needs_restart` is running an older `run`, `cwd` or `env` than its
  config now has. Restart it to apply the change.

## Adding a command to a project

For "add this app as a command on <project>", use `add-command` rather than editing YAML:

```sh
switchboard status <slug>                      # see the groups; pick one or name a new one
switchboard add-command <slug>/<group> --id dev --run "npm run dev" \
  [--cwd ~/path/to/app] [--name "dev"] [--no-autostart] [--restart-on-crash] [--env PORT=3000]
# a group that does not exist yet also needs: --group-name "Admin" --group-cwd ~/path/to/app
switchboard start <slug>/<group>/dev && switchboard wait <slug>/<group>/dev
```

- Check the run line works from a login shell in that folder first:
  `cd <folder> && zsh -lc '<run>'`. A pinned Node version needs
  `source ~/.nvm/nvm.sh && nvm use && …`, and Python tools often need their venv
  activated. [reference/setting-up-a-project.md](reference/setting-up-a-project.md)
  section 3 has the details.
- Servers get the default autostart. One-off tasks (tests, migrations) get
  `--no-autostart`.
- When the group comes from a repo, the command is written to that repo's
  `.switchboard/commands.yaml`. Mention it, because that is a change in their repo.

## Changing a command

To change an existing command (its run line, folder, env, autostart), use `edit-command`
rather than editing YAML. Only the fields you pass change:

```sh
switchboard edit-command <slug>/<group>/<cmd> --run "source ~/.nvm/nvm.sh && nvm use >/dev/null && npm run dev"
#   also --cwd, --name ("" = the default), --env K=V, --unset-env K,
#   --autostart / --no-autostart, --restart-on-crash / --no-restart-on-crash
switchboard restart <slug>/<group>/<cmd>   # if it's running: it shows needs_restart until then
```

- Check the new run line from a login shell in its folder first, as for `add-command`.
- It writes where the command is defined; for a group from a repo that's the repo's
  `.switchboard/commands.yaml`, so say so. The file is rewritten without its comments.
- Several commands (a dev server, its typecheck and tests) each need their own call.

## A project's home (keeping its settings in a repo)

A project's **home** is the repo folder that holds its `.switchboard/project.yaml` (and its
board, in `.switchboard/board/`). Projects have none until the author keeps their settings in
a repo. To see a project's home, or a suggestion for one:

```sh
switchboard keep <slug>            # its home, or a suggested folder and the alternatives
switchboard keep <slug> --json     # the same, for a script
```

The suggestion is the repo holding most of the project's folders, or, when each folder is its
own repo, the `e2e` folder, since it exercises the whole product.

- **Never keep a project in a repo on your own.** Propose the suggested folder (and name the
  alternatives) and ask the author: on the project's board if it has one, otherwise in the
  conversation. Only on their yes:
  `switchboard keep <slug> <folder>`, which writes `<folder>/.switchboard/project.yaml`.
- It refuses to move a home that already exists; the author does that in the app
  (Project › Keep Settings in Repo…).

## The board (decisions for the author)

A project's **board** is where you hand the project to its author: each decision is a card
with your recommendation and why, and the author's answers come straight back. It lives in the
project's home (`.switchboard/board/`) or, with no home yet, in Switchboard's app folder, where
its first card offers to keep it in a repo. The format is in `docs/board-contract.md` of the
Switchboard repo. Write it **only** with these commands, never by editing its files:

```sh
switchboard board add card.json [--ask] [--blocking]   # post a card (or - for stdin)
switchboard board ask <id> [--blocking]                # "I need this one now": pushes to the phone
switchboard board update <id> patch.json [--changed "what changed"]
switchboard board working "what you're doing" [--stage <step>]
switchboard board step <id> [done|now|next|later] [--title …] [--label …] [--blurb …] [--doc file.md#M3]
                                                       # a step of the plan, in the sidebar: its state
                                                       # and its fields (only what you pass changes)
switchboard board step <id> --new --title … [--after <id>]   # add a step; --after also moves one
switchboard board project [--name …] [--meta …] [--intro …] [--steps-label …]   # the board's own name and intro
switchboard board kept <id> [--commit <sha>]           # a decision's work is done
switchboard board decide <id>… [--pick o] [--note …]   # the author decided in chat, not on the board
switchboard board seen <id>…                           # the author has seen these (cards with no question)
switchboard board answers --wait                       # keep this running in the background
```

- **A card needs** `id`, `stage` (a step's id) and `title`. The question goes **inside
  `decision`** and pictures **inside `body`**; `add` and `update` refuse anything else and say
  where it goes:

  ```json
  { "id": "film-done", "stage": "m3", "title": "The film is done", "summary": "One paragraph.",
    "body": { "video": "~/Downloads/walkthrough.mp4" },
    "decision": { "q": "Is it good enough to review the PRs against?", "basis": "taste",
      "why": ["Why I recommend it, a paragraph each."],
      "options": [{ "o": "good", "label": "Good", "rec": true, "then": "Review the PRs against it." },
                  { "o": "redo", "label": "Re-record it" }] } }
  ```

  Each option has an `o` (a short id) and a `label`; one has `rec: true` and a `then`. `basis` is
  `evidence` or `taste`. When you're unsure, put `unsure: { why, where }` in `decision` instead of
  recommending. Pictures are `body.image`, `body.gallery: [{ src, caption }]`, `body.video`.
  `update` merges fields, so to fix a card's shape, `remove` it and `add` it again.
- **Images and videos: point at the file where it is** (`"image": "~/Downloads/shot.png"`). Don't
  copy it into the board or run a server for it; Switchboard shows any image a card names, and
  the phone does too while it's on the Mac's network.
- **Obvious calls don't get a card:** `switchboard board settle <id> "the call"` lists it as
  going ahead unless the author objects.
- **`--blocking` only when you truly can't go on** without the answer.
- **Keep the steps true.** When the plan or the product changes, change the step with `step`
  (its title, blurb, label, or the plan section `--doc` points at, which is checked). `""` returns
  a field to its default; an unknown id is an error, not a new step.
- **One verb per thing:** cards are `add`, `update`, `remove`; steps are `step` (`--new`,
  `--remove`). Never edit the board's files, including `board.json`.
- **When the author decides in the conversation**, record it with `decide` (their pick, or the
  recommendation they agreed to), so the Record and What's owed stay true; then `kept` once built.
- **Read answers as they come** (`answers --wait` returns when the author sends); never read them
  with the output hidden, which marks them read unseen. While the author is testing the board,
  ask whether what arrived is a real answer before acting on it.

## Setting a product up as a project

When asked to add a product to Switchboard ("set up X in Switchboard", "add its dev
servers"), follow [reference/setting-up-a-project.md](reference/setting-up-a-project.md).
It is a checklist: find the commands, check they run from a login shell, add the icon and
theme, validate, install and verify. The config format is in
[reference/projects.md](reference/projects.md), and themes are in
[reference/theming.md](reference/theming.md).

The short version:

```sh
# write config.yaml in a scratch folder (see reference/projects.md), then:
switchboard validate <folder>
switchboard install <folder>        # copies it into the app's Projects folder; the app picks it up
switchboard start <slug> && switchboard wait <slug>/<group>/<server>
switchboard status <slug>
```

Installed projects live in `~/Library/Application Support/Switchboard/Projects/<slug>/` on
macOS and `~/.local/share/Switchboard/Projects/<slug>/` on Linux. Edit `config.yaml` there;
Switchboard reloads it within a second without stopping anything. Do not write
`.switchboard/` folders into the author's repos unless asked.

## On Linux

- **Installing.** The command is built from the repo: `cargo install --path crates/sb-cli
  --locked` in `~/Work/switchboard` (it lands in `~/.cargo/bin`). This skill is a link,
  `~/.claude/skills/switchboard` → `~/Work/switchboard/skills/switchboard`. If either is
  missing, say so and ask before building.
- **The host.** `switchboard serve` holds the engine and the commands; quitting the window
  stops nothing. It logs to `~/.local/share/Switchboard/serve.log`. A systemd user unit
  (`switchboard.service`) is used when installed, else `start` launches `serve` in the
  background. Don't kill `serve`: it takes every running command down with it.
- **The window.** `switchboard ui` shows the projects, groups and commands with their
  status, and up to four live terminals. Your `start`, `stop` and `restart` show there.
- **A command's terminal in yours:** `switchboard attach <slug>/<group>/<cmd>` (Ctrl+]
  detaches and leaves it running). Use it to answer a prompt; use `logs` to read.
- **Linux differences go in overlays.** A shared `.switchboard/project.yaml` must not change
  for Linux. Put what differs (paths, `nvm` → `mise`, hidden Xcode builds) in
  `.switchboard/project.linux.yaml` beside it: groups and commands by id, only the fields
  that change, `hidden: true` to drop one. `project.local.yaml` does the same for one
  machine. `switchboard validate <repo folder>` lists the overlays it applied.
