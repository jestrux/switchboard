# 5. Release

**Status: Ready when the author wants to distribute.** Next step: confirm with the author whether a paid Apple Developer account and a Developer ID certificate exist. Nothing else here can start without them.

## State today

- Debug builds, signed with an **Apple Development** certificate by `scripts/build-app.sh`.
  That keeps folder permissions across rebuilds on the author's Mac. It is not distributable.
- The app is unsandboxed and Hardened Runtime is off (`app/project.yml`).
- The Rust core builds for the host architecture only in debug. `scripts/build-core.sh`
  without `--debug` builds a universal library when both targets are installed.
- The repo `jestrux/switchboard` is private.

## To ship to other people

1. **Developer ID Application certificate** (needs a paid Apple Developer account).
2. **Hardened Runtime** on. The app spawns arbitrary child processes through a PTY; check
   that works under the hardened runtime and add only the entitlements it proves to need.
3. **Release configuration**: `scripts/package.sh Release`. The Debug-only menu item
   "Install Sample Project" reads a path baked in from the source tree (`#filePath` in
   `EngineStore.installSampleProject`), so it must stay out of release builds.
4. **Notarize and staple**, then package as a DMG or zip.
5. **Version.** `CFBundleShortVersionString` is `0.1.0` in `app/project.yml`; the Rust
   workspace version is `0.1.0`. Decide how they move together.
6. Decide on updates (Sparkle or manual downloads).

Related PRD open questions: bundle id (currently `dev.waky.switchboard`) and sandboxing.
See [open-questions.md](open-questions.md).

## Before making the repo public

- **Personal details in media.** The walkthrough's terminal beat shows the author's
  username and Mac name in the shell prompt, and footers show paths under `~/Documents/web`.
- **Sample configs** name real local folders and products. Fine for the author, worth a
  look.
- **`handoff/` and `plans/`** are working material. Delete them first, as `PLAN.md` says.
- **Licence.** `Cargo.toml` says MIT; there is no `LICENSE` file.
- **Third-party notices.** SwiftTerm and the Rust crates.
- Run a secret scan over the full history, not only the working tree.
