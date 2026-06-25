#!/usr/bin/env bash
set -euo pipefail

ROOT="${TMPDIR:-/tmp}/nuked-demo"
OLD_STAMP="202401010101"
NEW_STAMP="$(date +%Y%m%d%H%M)"

rm -rf "$ROOT"
mkdir -p \
    "$ROOT/workspace-api/node_modules/.cache" \
    "$ROOT/docs-site/node_modules/vite" \
    "$ROOT/ml-tools/.venv/bin" \
    "$ROOT/fresh-app/node_modules/react"

printf '{"scripts":{"dev":"node server.js"}}\n' > "$ROOT/workspace-api/package.json"
printf 'console.log("api")\n' > "$ROOT/workspace-api/server.js"
printf '{"devDependencies":{"vite":"latest"}}\n' > "$ROOT/docs-site/package.json"
printf '# docs\n' > "$ROOT/docs-site/README.md"
printf '[project]\nname = "ml-tools"\n' > "$ROOT/ml-tools/pyproject.toml"
printf 'home = /usr/bin/python3\n' > "$ROOT/ml-tools/.venv/pyvenv.cfg"
printf '{"dependencies":{"react":"latest"}}\n' > "$ROOT/fresh-app/package.json"

dd if=/dev/zero of="$ROOT/workspace-api/node_modules/api.bin" bs=1024 count=64 >/dev/null 2>&1
dd if=/dev/zero of="$ROOT/docs-site/node_modules/docs.bin" bs=1024 count=48 >/dev/null 2>&1
dd if=/dev/zero of="$ROOT/ml-tools/.venv/model.bin" bs=1024 count=36 >/dev/null 2>&1
dd if=/dev/zero of="$ROOT/fresh-app/node_modules/fresh.bin" bs=1024 count=24 >/dev/null 2>&1

touch -t "$OLD_STAMP" \
    "$ROOT/workspace-api/package.json" \
    "$ROOT/workspace-api/server.js" \
    "$ROOT/docs-site/package.json" \
    "$ROOT/docs-site/README.md" \
    "$ROOT/ml-tools/pyproject.toml" \
    "$ROOT/ml-tools/.venv/pyvenv.cfg"

touch -t "$NEW_STAMP" "$ROOT/fresh-app/package.json"

printf '%s\n' "$ROOT"
