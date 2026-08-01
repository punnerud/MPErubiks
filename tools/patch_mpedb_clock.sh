#!/usr/bin/env bash
# Trunk post_build hook: mpedb-core (0.1.5+) declares its host imports in
# wasm import module "mpedb"; wasm-bindgen turns that into a bare ES
# import (`import * as importN from "mpedb"`) which no browser can
# resolve - the app would never start. Rewrite it into a local object
# supplying the embedder contract (the browser clock). Fails loudly if
# the import statement is not found exactly once.
set -euo pipefail
cd "${TRUNK_STAGING_DIR:?}"
glue=$(ls ./cube-app-*.js | head -1)
count=$(grep -cE "import \* as [A-Za-z0-9_]+ from ['\"]mpedb['\"]" "$glue" || true)
if [ "$count" -ne 1 ]; then
  echo "patch_mpedb_clock: expected exactly 1 mpedb import in $glue, found $count" >&2
  exit 1
fi
perl -0pi -e "s/import \* as ([A-Za-z0-9_]+) from ['\"]mpedb['\"];?/const \$1 = { mpedb_host_now_ms: () => Date.now() };/" "$glue"
if grep -qE "from ['\"]mpedb['\"]" "$glue"; then
  echo "patch_mpedb_clock: rewrite failed, import still present" >&2
  exit 1
fi
echo "patch_mpedb_clock: replaced the bare mpedb import with the embedder clock"
