#!/usr/bin/env bash
# Browse docs/ as a site. The Markdown files are the source; this only renders them.
#   scripts/docs.sh          dev server on http://localhost:5199
#   scripts/docs.sh build    static build in docs-site/dist
set -euo pipefail
cd "$(dirname "$0")/../docs-site"
[ -d node_modules ] || npm install
if [ "${1:-}" = "build" ]; then npm run build; else npm run dev -- --open; fi
