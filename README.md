# Switchboard

A native Mac app that starts, watches and stops a project's dev commands from one
window, with each command running in its own folder. A project is a logical grouping
of commands, not a directory. See `handoff/PRD.md` for the full spec.

```
crates/sb-core      Rust engine: config, supervisor, PTY sessions, file watcher
crates/sb-cli       `sb` command-line front end
crates/sb-ffi       UniFFI bridge used by the Mac app
app/                SwiftUI app (Switchboard.app)
samples/kinpay      Reference project (config.yaml + icon.png)
```

Projects live in `~/Library/Application Support/Switchboard/Projects/<slug>/config.yaml`.

## Build and run the CLI (M1)

Requires a Rust toolchain (`curl https://sh.rustup.rs | sh`).

```sh
cargo build --release -p sb-cli          # binary at target/release/sb
cargo test -p sb-core                    # config + process-tree tests

sb validate samples/kinpay               # check a config.yaml
sb install samples/kinpay                # copy it into the Projects directory
sb list                                  # projects, commands, live status
sb run kinpay                            # autostart commands, prefixed output; Ctrl+C stops all
sb run kinpay backend                    # one group
sb run kinpay backend/migrate            # one command in the foreground (interactive)
sb run samples/kinpay                    # a project folder that is not installed
sb cleanup                               # kill sessions left behind by a crash
```

Commands run through `$SHELL -lc "<run>"` in their own PTY and process group. Stop
sends SIGINT to the group, SIGTERM after 3 s, SIGKILL after a further 3 s.
