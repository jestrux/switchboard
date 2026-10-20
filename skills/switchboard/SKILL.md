---
name: switchboard
description: Run, watch and debug a project's dev servers in the Switchboard Mac app with the `switchboard` command. Start, stop or restart commands, wait until a server is ready, read its output, and set a product up as a Switchboard project. Use when the user mentions Switchboard, or asks to start, run, restart or check a dev server or read its logs, and `switchboard` is installed.
---

# Switchboard

Switchboard is a Mac app that runs a project's dev commands (servers, workers, watchers,
one-off tasks) in its own terminals. The author can see every one of them there. The
`switchboard` command lets you drive that app. Commands you start this way run in the
app, not in your shell, so they keep running after you finish. The author can watch
them, and you can read their output later.

Check that it is installed: `command -v switchboard`. If it is missing, run
`/Applications/Switchboard.app/Contents/Helpers/switchboard setup`. It links the command
onto the PATH and this skill into `~/.claude/skills`, the same as the app's
**Switchboard › Install switchboard Command…**. If Switchboard.app is not in
Applications, ask the author to install it. Do not build the command yourself.

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
switchboard start <slug>                  # the autostart commands (launches the app if needed)
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
  look.

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
switchboard board step <id> done|now|next|later        # the plan's steps, in the sidebar
switchboard board kept <id> [--commit <sha>]           # a decision's work is done
switchboard board decide <id>… [--pick o] [--note …]   # the author decided in chat, not on the board
switchboard board answers --wait                       # keep this running in the background
```

- **A card needs** `id`, `stage` (a step's id) and `title`; a decision has `q`, `options` (one
  with `rec: true` and a `then`), `why`, and `basis` (`evidence` or `taste`). When you're unsure,
  say so with `unsure: { why, where }` instead of recommending.
- **Obvious calls don't get a card:** `switchboard board settle <id> "the call"` lists it as
  going ahead unless the author objects.
- **`--blocking` only when you truly can't go on** without the answer.
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

Installed projects live in `~/Library/Application Support/Switchboard/Projects/<slug>/`.
Edit `config.yaml` there; the app reloads it within a second without stopping anything.
Do not write `.switchboard/` folders into the author's repos unless asked.
