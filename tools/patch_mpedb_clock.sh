#!/usr/bin/env bash
# Trunk post_build hook: mpedb-core (0.1.5+) requires the EMBEDDER to
# supply the host import `mpedb.mpedb_host_now_ms` (the browser clock).
# wasm-bindgen only populates the `wbg` import module, so we WRAP the
# glue's __wbg_get_imports (module-scope function bindings are mutable;
# the wrapper runs at module evaluation, before init). Fails loudly if
# the anchor is missing - a silent miss would be a runtime LinkError.
set -euo pipefail
cd "${TRUNK_STAGING_DIR:?}"
glue=$(ls ./cube-app-*.js | head -1)
if ! grep -q "__wbg_get_imports" "$glue"; then
  echo "patch_mpedb_clock: anchor __wbg_get_imports missing in $glue" >&2
  exit 1
fi
if grep -q "mpedb_host_now_ms" "$glue"; then
  echo "patch_mpedb_clock: already patched"
  exit 0
fi
cat >> "$glue" <<'JS'

// mpedb embedder contract: supply the wall clock as a host import.
const __mpedb_orig_get_imports = __wbg_get_imports;
__wbg_get_imports = function () {
    const imports = __mpedb_orig_get_imports();
    imports.mpedb = { mpedb_host_now_ms: () => Date.now() };
    return imports;
};
JS
echo "patch_mpedb_clock: wrapped __wbg_get_imports in $glue"
