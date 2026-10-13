# 5. Release

**Status: Signing, notarization and packaging are done (2026-10-13).** `scripts/release.sh`
produces a notarized zip and, with `--publish`, a GitHub release. Next step: decide on
updates and on how the app and Rust versions move together.

## State today

- **`scripts/release.sh`** builds a universal Release archive with the hardened runtime,
  signs it with Developer ID, submits it to Apple's notary service, staples the ticket and
  zips it to `dist/Switchboard-<version>.zip`. Run end to end on 2026-10-13; Gatekeeper
  reports the result as "Notarized Developer ID".
- **The script owns the version.** `scripts/release.sh patch|minor|major|X.Y.Z` rewrites
  `CFBundleShortVersionString` and the build number in `app/project.yml` and the workspace
  version in `Cargo.toml`, so the app and the Rust crates always move together. With
  `--publish` it commits the bump as "Release X.Y.Z", pushes, and creates the release on
  that commit with generated notes. It refuses a dirty tree and an already-released
  version, and restores the version files if the build fails. `--dry-run` previews.
  The bump-and-publish path was dry-run only on 2026-10-13; 0.1.0 was published by hand.
- **No certificate is installed and no password is stored.** The script uses the developer
  account Xcode is signed in to. Apple creates and holds a cloud-managed Developer ID
  certificate (`-allowProvisioningUpdates`), and notarization goes through the same account
  (`destination: upload`, then `-exportNotarizedApp`).
- **The hardened runtime needs no entitlements.** The notarized build started and stopped a
  dev server through its PTY.
- Day-to-day Debug builds are still signed with the **Apple Development** certificate by
  `scripts/build-app.sh`, which keeps folder permissions across rebuilds.
- Agreement errors ("PLA Update available", "A required agreement is missing") mean a
  pending agreement at developer.apple.com/account. They clear a minute or so after it is
  accepted.
- The Debug-only menu item "Install Sample Project" reads a source-tree path and stays out
  of Release builds.
- The repo `jestrux/switchboard` is private, so a release is visible only to people with
  access.

## Still to decide

1. **Updates**: Sparkle or manual downloads.
2. A DMG instead of a zip, if a drag-to-Applications window is wanted.

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
