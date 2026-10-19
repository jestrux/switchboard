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

Check that it is installed: `command -v switchboard`. If it is missing, ask the author
to choose **Switchboard › Install switchboard Command…** in the app. Do not build it
yourself.

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
