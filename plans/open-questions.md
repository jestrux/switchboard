# Open questions

Product decisions not yet made. Ask the author rather than guessing.

## From the PRD

| Question | Status | Blocks |
|---|---|---|
| Bundle ID | Using `dev.waky.switchboard`. Never confirmed as final. | [Release](05-release.md) |
| Sandboxing | Shipping unsandboxed, which was the PRD's recommendation. Not formally decided. | [Release](05-release.md) |
| Shared commands across projects (one Postgres used by several) | Deferred. A command belongs to one project and at most one group. | Nothing now |
| Should `sb` control the running app (`sb start kinpay` over a local socket)? | Not built. Today `sb run` starts its own processes and `sb list` only reads the app's session file. | Nothing now |
| Per-project `.env` file alongside inline `env` | Not built. | Nothing now |

## Raised during the build

- **Two groups in one folder.** Kinpay's Frontend and Tunnels groups share a folder, but a
  folder has one `commands.yaml`. Blocks [splitting into fragments](02-split-into-fragments.md).
- **YAML comments.** Rewriting a config from the app drops comments. Matters once
  [editing sheets](01-editing-sheets.md) write configs routinely.
- **Is Export still wanted** now that repo files and `sb import` exist?
  See [M5](03-m5-remaining.md).
- **Do the editing sheets need a design module** from Claude Design, or should they follow
  the existing dialog card? See [editing sheets](01-editing-sheets.md).
- **Public or private repo**, and when. See [Release](05-release.md).
- **Should circles dev servers autostart with Kinpay?** They are opt-in today.
- **A custom font.** The design handoff asks whether to bundle one; everything uses SF Pro
  and SF Mono.
- **Engine fields the design assumed.** `HANDOFF.md` lists data the UI wants. Backoff
  deadline and start-now are done. "Leftover sessions keep their port" is stated in the
  dialog copy but was never verified.
