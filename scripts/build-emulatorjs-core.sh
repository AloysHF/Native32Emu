#!/usr/bin/env bash
#
# Build the Native32Emu libretro core as an EmulatorJS `.data` bundle.
#
# This follows EmulatorJS's own pipeline (https://emulatorjs.org/docs4devs/buildingraw):
# compile the core for wasm32-unknown-emscripten, link it into the
# EmulatorJS/RetroArch fork with Makefile.emulatorjs, and package the result as
# `<core>-wasm.data` / `<core>-legacy-wasm.data` containing core.json,
# build.json and license.txt.
#
# Requirements: an activated emsdk (`emmake` in PATH), p7zip (`7z`), git, and a
# Rust staticlib built with:
#   cargo build -p native32emu-libretro --target wasm32-unknown-emscripten \
#     --profile wasm-release
#
# Usage:
#   scripts/build-emulatorjs-core.sh [--core <libnative32emu.a>] [--out <dir>]
#                                    [--work-dir <dir>] [--retroarch-ref <sha>]

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORE_NAME="native32emu"
RETROARCH_REPO="${RETROARCH_REPO:-https://github.com/EmulatorJS/RetroArch.git}"
RETROARCH_REF="${RETROARCH_REF:-86f72a39736b1fa351ca296d16f568e99e11a34f}"
MIN_EJS_VERSION="${MIN_EJS_VERSION:-4.2.3}"

CORE_LIB="$REPO_ROOT/target/wasm32-unknown-emscripten/wasm-release/libnative32emu.a"
OUT_DIR="$REPO_ROOT/out/emulatorjs"
WORK_DIR="$REPO_ROOT/target/emulatorjs-build"

usage() {
    sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --core) CORE_LIB="${2:?--core needs a path}"; shift 2 ;;
        --out) OUT_DIR="${2:?--out needs a path}"; shift 2 ;;
        --work-dir) WORK_DIR="${2:?--work-dir needs a path}"; shift 2 ;;
        --retroarch-ref) RETROARCH_REF="${2:?--retroarch-ref needs a commit}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 1 ;;
    esac
done

for tool in emmake emcc 7z zip git; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "error: '$tool' not found. Activate emsdk and install p7zip." >&2
        exit 1
    }
done

[[ -f "$CORE_LIB" ]] || {
    echo "error: core staticlib not found: $CORE_LIB" >&2
    echo "Build it first with:" >&2
    echo "  cargo build -p native32emu-libretro --target wasm32-unknown-emscripten --profile wasm-release" >&2
    exit 1
}

WORK_DIR="$(mkdir -p "$WORK_DIR" && cd "$WORK_DIR" && pwd)"
RETROARCH_DIR="$WORK_DIR/RetroArch"
EMULATORJS_DIR="$WORK_DIR/EmulatorJS"
BUILD_DIR="$RETROARCH_DIR/emulatorjs"

fetch_retroarch() {
    if [[ ! -d "$RETROARCH_DIR/.git" ]]; then
        git init "$RETROARCH_DIR"
        git -C "$RETROARCH_DIR" remote add origin "$RETROARCH_REPO"
    fi
    if ! git -C "$RETROARCH_DIR" cat-file -e "$RETROARCH_REF^{commit}" 2>/dev/null; then
        git -C "$RETROARCH_DIR" fetch --depth 1 origin "$RETROARCH_REF"
    fi
    git -C "$RETROARCH_DIR" checkout --detach "$RETROARCH_REF"
}

echo "==> Fetching EmulatorJS/RetroArch @ $RETROARCH_REF"
fetch_retroarch

# build-emulatorjs.sh writes its output to ../../EmulatorJS/data/cores relative
# to RetroArch/emulatorjs, i.e. the sibling EmulatorJS directory here.
mkdir -p "$EMULATORJS_DIR/data/cores"
rm -f "$EMULATORJS_DIR/data/cores/${CORE_NAME}"*.data

echo "==> Linking $CORE_LIB into the EmulatorJS runtime"
cp "$CORE_LIB" "$BUILD_DIR/${CORE_NAME}_libretro_emscripten.bc"

BUILD_START="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
(
    cd "$BUILD_DIR"
    echo "==> Building wasm variant"
    emmake ./build-emulatorjs.sh
    echo "==> Building legacy (WebGL1) variant"
    emmake ./build-emulatorjs.sh --clean --legacy
)
BUILD_END="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

WASM_DATA="$EMULATORJS_DIR/data/cores/${CORE_NAME}-wasm.data"
LEGACY_DATA="$EMULATORJS_DIR/data/cores/${CORE_NAME}-legacy-wasm.data"
for file in "$WASM_DATA" "$LEGACY_DATA"; do
    [[ -f "$file" ]] || { echo "error: expected $file was not produced" >&2; exit 1; }
done

echo "==> Packaging metadata"
rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR/cores/reports"

cp "$WASM_DATA" "$LEGACY_DATA" "$OUT_DIR/cores/"

cat > "$OUT_DIR/core.json" <<EOF
{
  "name": "Native32",
  "extensions": ["smf", "sgm", "zip"],
  "options": {
    "defaultWebGL2": true
  },
  "save": false,
  "license": "LICENSE",
  "repo": "https://github.com/AloysHF/Native32Emu"
}
EOF

cat > "$OUT_DIR/build.json" <<EOF
{
  "core": "${CORE_NAME}",
  "minimumEJSVersion": "${MIN_EJS_VERSION}",
  "buildStart": "${BUILD_START}",
  "buildEnd": "${BUILD_END}",
  "retroarchRef": "${RETROARCH_REF}"
}
EOF

cp "$REPO_ROOT/LICENSE" "$OUT_DIR/license.txt"

cat > "$OUT_DIR/cores/reports/${CORE_NAME}.json" <<EOF
{
  "core": "${CORE_NAME}",
  "buildStart": "${BUILD_START}",
  "buildEnd": "${BUILD_END}",
  "options": {
    "defaultWebGL2": true
  }
}
EOF

# EmulatorJS keeps core.json/build.json/license.txt inside each .data archive.
for file in "$OUT_DIR/cores/${CORE_NAME}-wasm.data" "$OUT_DIR/cores/${CORE_NAME}-legacy-wasm.data"; do
    (
        cd "$OUT_DIR"
        7z a -t7z -y "$file" core.json build.json license.txt >/dev/null
    )
done

cat > "$OUT_DIR/README.md" <<'EOF'
# Native32Emu — EmulatorJS core

Drop-in core bundle for [EmulatorJS](https://emulatorjs.org).

## Install

Copy `cores/` into your EmulatorJS `data/` directory so the result looks like:

```
data/
  cores/native32emu-wasm.data
  cores/native32emu-legacy-wasm.data
  cores/reports/native32emu.json
```

## Use

```html
<script>
  EJS_player = "#game";
  EJS_core = "native32emu";
  EJS_gameUrl = "games/your-game.zip";
  EJS_pathtodata = "data/";
</script>
<script src="data/loader.js"></script>
```

The core accepts `.smf`, `.sgm` and `.zip` game packages.
EOF

ZIP="$OUT_DIR/../native32-emu-emulatorjs.zip"
rm -f "$ZIP"
(
    cd "$OUT_DIR"
    zip -q -r "$ZIP" cores core.json build.json license.txt README.md
)

# Flat copies for release pages / quick downloads.
cp "$OUT_DIR/cores/${CORE_NAME}-wasm.data" "$OUT_DIR/"
cp "$OUT_DIR/cores/${CORE_NAME}-legacy-wasm.data" "$OUT_DIR/"
cp "$OUT_DIR/cores/reports/${CORE_NAME}.json" "$OUT_DIR/"

echo "==> Done"
echo "    bundle:  $(cd "$(dirname "$ZIP")" && pwd)/$(basename "$ZIP")"
echo "    staging: $OUT_DIR"
