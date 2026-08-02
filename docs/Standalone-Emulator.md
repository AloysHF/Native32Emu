# Standalone Emulator

This guide covers installing and running the standalone `native32-emu` binary,
loading individual games or ZIP packages, keyboard controls, cheats, display
scaling, and every command-line option.

## Installation

Download the latest standalone binary for your platform from the
[Releases](https://github.com/jiangxincode/Native32Emu/releases) page.

You can also build it from source:

```bash
cargo build -p native32emu --release
```

The binary is produced at `target/release/native32-emu` (`.exe` on Windows).

## Loading Games

The standalone emulator accepts `.smf`, `.sgm`, `.ssl`, and `.zip` files.

```bash
# Load a game directly
native32-emu path/to/game.smf

# Load a ZIP package containing FHUI.smf
native32-emu path/to/game.zip
```

### ZIP Mode

When loading from a `.zip` file, the emulator starts the FHUI menu. Selecting a
game launches it; pressing **ESC** during gameplay returns to the menu. Pressing
**ESC** on the menu itself exits the emulator. When loading a `.smf` file
directly, **ESC** exits as usual. This matches the RetroArch core's back-action
behavior, where RetroPad **Select** performs the same return-or-exit operation.

You can always print the built-in help with:

```bash
native32-emu --help
```

## Synopsis

```text
native32-emu [OPTIONS] <GAME_PATH>
```

## Options

| Option | Value | Default | Description |
|---|---|---|---|
| `<GAME_PATH>` | path | *required* | Path to the game file (`.smf`, `.sgm`, `.ssl`, or `.zip`). For `.zip` files, extracts and loads FHUI.smf automatically. |
| `-s, --scale <N>` | `1`–`16` | `1` | Integer scaling factor for the window. |
| `-f, --fullscreen` | flag | off | Run in borderless fullscreen at the desktop resolution. |
| `-v, --volume <N>` | `0`–`100` | `100` | Volume level (`0` = mute, `100` = original). |
| `--swap-ab` | flag | off | Swap the A and B button actions. |
| `--auto-skip-cutscenes` | flag | off | Automatically skip logo/intro/cutscene videos instead of playing them. |
| `--debug` | flag | off | Enable debug/development mode. |
| `--remap <KEYCODE:KEY>` | string | — | Remap a Native32 keycode to a physical key. Repeatable. |
| `--cheat <RULE>` | string | — | Enable a cheat rule. Repeatable. |
| `--debug-cheats` | flag | off | Periodically log VM variables and sprites to help build cheat rules. |
| `--cheat-debug-interval <N>` | integer | `30` | Frames between cheat target debug logs. |
| `--cheat-debug-filter <GLOB>` | string | all | Only log VM variable names matching a case-sensitive glob (`*` and `?`). |
| `-S, --screenshot <PATH>` | path | — | Render some frames, save a PNG screenshot, then exit. |
| `--screenshot-frames <N>` | integer | `30` | Number of frames to run before the screenshot is taken. |
| `--show-gamepad` | flag | off | Draw an on-screen virtual gamepad overlay showing pressed keys. |
| `--repeat-delay <N>` | integer | `12` | Frames a held key waits before auto-repeat starts (see below). |
| `--repeat-period <N>` | integer (≥1) | `3` | Frames between auto-repeat pulses once repeating (see below). |
| `--filter <NAME>` | string | `nearest` | Pixel scaling filter: `nearest`, `bilinear`, `bicubic`, or `xbrz` (pixel-art). |

`--screenshot-frames` only has an effect together with `--screenshot`.

## Default Key Mappings

| Native32 Keycode | Physical Key | Action |
|---|---|---|
| `0x0200` | ← Left | Left |
| `0x0400` | → Right | Right |
| `0x1c00` | ↑ Up | Up |
| `0x1e00` | ↓ Down | Down |
| `0x4000` | Z | A |
| `0x8800` | X | B / Menu |

## Key Remapping

Use `--remap <KEYCODE:KEY>` to bind a Native32 keycode to a different physical
key. The keycode is hexadecimal (with or without the `0x` prefix) and must be
one of the keycodes in the table above. The key name is case-insensitive.

```bash
# Map the A button (0x4000) to the spacebar
native32-emu --remap "0x4000:space" game.smf

# Map directions to WASD (repeat the flag for each binding)
native32-emu \
  --remap "0x1c00:w" --remap "0x1e00:s" \
  --remap "0x0200:a" --remap "0x0400:d" game.smf
```

Supported key names: `a`–`z`, `0`–`9`, `space`, `enter`/`return`,
`left`, `right`, `up`, `down`, `escape`/`esc`. Invalid entries are ignored
with a warning.

## Cheats

Verified game-specific rules are listed in [Game Cheat Codes](Cheat-Codes.md).

Use `--cheat <RULE>` to apply a cheat rule. The option is repeatable, and rules
are applied every emulation tick after normal game logic. The RetroArch core
accepts the same rules through its emulator-handled cheat interface.

Supported rule forms:

- `var:<name>=<value>` — force a Native32 VM variable.
- `sprite:<name>.<field>=<value>` or `movie:<name>.<field>=<value>` — force a movie sprite field. Supported fields: `x`, `y`, `depth`, `frame`, `visible`, `playing`.
- `frame:goto=<n>` — force the main timeline to jump to a frame.
- `frame:playing=<bool>` — force the main timeline play/pause state.

Boolean values accept `1`/`0`, `true`/`false`, `on`/`off`, and `yes`/`no`.

To discover usable targets, run with `--debug-cheats`. The log lists current VM variables and sprite names/fields that can be used in `var:` and `sprite:` rules. Use `--cheat-debug-filter` to restrict variable names with a case-sensitive glob.

```bash
native32-emu --cheat "var:lives=99" game.smf
native32-emu --cheat "sprite:player.visible=0" game.smf
native32-emu --debug-cheats --cheat-debug-filter "p_*" game.smf
```

## Keypad Auto-Repeat (`--repeat-delay` / `--repeat-period`)

The original hardware keypad does not report a held key as pressed on every
frame. It emits an initial press, pauses for a short delay, then auto-repeats at
a fixed rate. Several games depend on the gap this produces.

The clearest example is the walk/run mechanic in some action games: holding a
direction makes the character walk, and the pause-then-repeat from auto-repeat
is what the game detects to break into a run. Without it, the character would
walk forever and never run.

`native32-emu` reproduces this behaviour. Both values are measured in frames at
30fps:

- `--repeat-delay` — how long a key must be held before it starts repeating.
  Default `12` (~0.4s).
- `--repeat-period` — the interval between repeats once repeating. Default `3`
  (~0.1s). Must be at least `1`.

The defaults match the hardware feel. You can tune them per preference:

```bash
# Trigger the run sooner and repeat faster
native32-emu --repeat-delay 10 --repeat-period 2 game.smf

# Slower, more deliberate repeats (e.g. for menus)
native32-emu --repeat-delay 16 --repeat-period 6 game.smf
```

Note: a very large `--repeat-delay` can make hold-to-run mechanics hard or
impossible to trigger, because the game's detection window may close before the
first repeat arrives.

## Scaling Filter (`--filter`)

The `--filter` option controls how the native-resolution framebuffer (typically
320×240) is upscaled to the display window size.

| Filter | Algorithm | Character |
|---|---|---|
| `nearest` | Nearest-neighbor | Sharp pixel edges, classic retro look (default) |
| `bilinear` | Bilinear interpolation | Smooth edges, reduces staircase artifacts |
| `bicubic` | Catmull-Rom bicubic | Sharper than bilinear, better edge preservation |
| `xbrz` | xBRZ pixel-art scaler | Smooth diagonals, sharp pixel edges, best for retro games |

The filter has no effect at `--scale 1` (no upscaling needed). At higher scale
factors the difference is most visible on diagonal lines and text:

```bash
# Sharp pixels (default)
native32-emu --scale 4 game.smf

# Smooth bilinear interpolation
native32-emu --scale 4 --filter bilinear game.smf

# Sharper bicubic (Catmull-Rom)
native32-emu --scale 4 --filter bicubic game.smf

# xBRZ pixel-art scaler (smooth diagonals, sharp edges)
native32-emu --scale 4 --filter xbrz game.smf
```

## Examples

```bash
# Basic usage
native32-emu path/to/game.smf

# Load from ZIP archive (auto-extracts and loads FHUI.smf)
native32-emu path/to/game.zip

# 2x scaling with 80% volume
native32-emu --scale 2 --volume 80 path/to/game.smf

# Fullscreen
native32-emu --fullscreen game.smf

# Swap A/B and skip intro videos
native32-emu --swap-ab --auto-skip-cutscenes game.smf

# Show the on-screen gamepad overlay
native32-emu --show-gamepad game.smf

# Take a screenshot after 30 frames and exit
native32-emu --screenshot screenshot.png --screenshot-frames 30 game.smf

# Fullscreen with smooth bilinear scaling
native32-emu --fullscreen --filter bilinear game.smf
```
