# 2. Split a project into repo fragments

## Goal

Turn a project whose commands are written inline in `config.yaml` into one that composes
per-folder fragments: each group's commands move into `<group folder>/.switchboard/commands.yaml`
and the group becomes `from: <folder>`.

## Why it is needed

"Keep settings in the repo" (`Engine::adopt_home`) writes `project.yaml` to the home folder
but leaves commands inline. For the author's real projects that means one repo holds every
command, including those that belong to other repos. The point of the design was that each
folder owns its own bits.

## Behaviour to build

- An action per project: **Split into Repo Folders…**, also offered right after Keep.
- Show a preview before writing: each group, the folder it will write to, the commands that
  move, and anything that cannot move.
- A group can be split when it has a `cwd` and all its commands run in that folder. A
  command with its own different `cwd` either stays inline in the project or moves with a
  relative `cwd` when it is inside the group folder. Ungrouped commands stay in the project.
- Per-project settings that should not be shared, such as a different `autostart`, become
  `overrides` in `project.yaml` rather than values in the fragment. Simplest first version:
  write the command as it is and add no overrides.
- If a folder already has a `commands.yaml`, merge by command id and report conflicts
  instead of overwriting.
- Never write into a repo without the confirmation.

## What exists

- `crates/sb-core/src/fragment.rs`: `Fragment::load/save`, `ProjectConfig::compose`,
  `CommandOverride`, `ProjectSeed::from_config/to_config`, `relative_to_home`.
- `Engine::promote_terminal` already writes a command into a fragment.
- `crates/sb-core/tests/fragment_tests.rs` has a two-repo workspace helper to extend.
- `sb validate <repo folder>` reports every fragment read, useful for verifying the result.

## Test on

A scratch workspace, as the existing tests do. Then, only when the author asks, on Kinpay:
six groups across `kinpay`, `kinpay-backend` and four `kinpay-circles` folders. Two Kinpay
groups (Frontend and Tunnels) share the folder `~/Documents/web/kinpay`, so two groups would
write to the same fragment. Decide how that is handled: one fragment per folder means
groups need a way to pick a subset, or the fragment format needs named sections. This is
the main design question in this item.
