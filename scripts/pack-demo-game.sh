#!/usr/bin/env bash
#
# Pack the Bloody Blade (赤刃) files of a local Native32 game collection into the
# ZIP that site/play.html loads by default.
#
# The game data is copyrighted and is not distributed with this repository, so
# `site/games/*.zip` is git-ignored. Run this locally (or point the Pages
# workflow at a URL via the NATIVE32_DEMO_GAME_URL repository variable) to
# enable the one-click online demo.
#
# Usage:
#   scripts/pack-demo-game.sh [game-dir] [output-zip]
#
#   game-dir     defaults to $NATIVE32_GAME_DIR or ./tmp/native32_game
#   output-zip   defaults to ./site/games/ebblade.zip

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GAME_DIR="${1:-${NATIVE32_GAME_DIR:-$REPO_ROOT/tmp/native32_game}}"
OUT_ZIP="${2:-$REPO_ROOT/site/games/ebblade.zip}"

LAUNCHER="$GAME_DIR/EACT/EBBLADE.smf"
LOGO="$GAME_DIR/NA32SSL/ENGLISH/LOGO/NALOGO.MPG"
SCENES="$GAME_DIR/NA32SSL/ENGLISH/BBLADE"

for path in "$LAUNCHER" "$LOGO" "$SCENES"; do
    [[ -e "$path" ]] || { echo "error: missing $path" >&2; exit 1; }
done

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

mkdir -p "$STAGE/EACT"
mkdir -p "$STAGE/NA32SSL/ENGLISH/LOGO"
mkdir -p "$STAGE/NA32SSL/ENGLISH/BBLADE"

cp "$LAUNCHER" "$STAGE/EACT/EBBLADE.smf"
cp "$LOGO" "$STAGE/NA32SSL/ENGLISH/LOGO/NALOGO.MPG"

# Only the scene scripts; save files from a local play session must not ship.
find "$SCENES" -maxdepth 1 -type f -iname '*.ssl' -exec cp {} "$STAGE/NA32SSL/ENGLISH/BBLADE/" \;
[[ -n "$(find "$STAGE/NA32SSL/ENGLISH/BBLADE" -type f -iname '*.ssl' -print -quit)" ]] || {
    echo "error: no .SSL scenes found in $SCENES" >&2
    exit 1
}

mkdir -p "$(dirname "$OUT_ZIP")"
rm -f "$OUT_ZIP"
(
    cd "$STAGE"
    zip -q -r "$OUT_ZIP" .
)

echo "Packed $(du -h "$OUT_ZIP" | cut -f1): $OUT_ZIP"
echo "site/play.html will load it as games/ebblade.zip."
