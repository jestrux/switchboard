#!/bin/sh
# Refresh the agent skill's reference pages from docs/. They are copies, not links, because
# the skill ships inside the app bundle. `cargo test -p sb-core` fails while they differ.
set -eu
cd "$(dirname "$0")/.."
for f in projects.md setting-up-a-project.md theming.md; do
  cp "docs/$f" "skills/switchboard/reference/$f"
done
echo "==> skills/switchboard/reference refreshed"
