# RetroArch Core

Native32Emu is available as a libretro core for RetroArch on Windows, Linux,
macOS, Android, and iOS. This guide covers installation, loading content,
supported frontend features, controls, core options, and cheats.

## Installation

### Online Updater

1. Open RetroArch.
2. Go to **Main Menu > Online Updater > Core Downloader**.
3. Select **Native32 (Native32Emu)**.

### Manual Installation

Download the core from the
[Releases](https://github.com/jiangxincode/Native32Emu/releases) page. Copy
`native32emu_libretro.dll` (`.so` on Linux or `.dylib` on macOS) to
RetroArch's `cores/` directory, and copy `native32emu_libretro.info` to its
`info/` directory.

## Loading Games

1. Open RetroArch and select **Load Core > Native32 (Native32Emu)**.
2. Select **Load Content**.
3. Choose a `.smf`, `.sgm`, `.ssl`, or `.zip` file.

## Mobile Platforms

The same libretro core architecture is available on mobile platforms, with
platform-specific installation requirements:

- [Android Libretro Core](Android-Libretro-Core.md)
- [iOS Libretro Core](iOS-Libretro-Core.md)

## Supported Features

- Video output using the XRGB8888 pixel format
- Stereo audio output with looping MP3 music and RAW PCM sound effects
- RetroPad input handling
- `.smf`, `.sgm`, `.ssl`, and `.zip` content loading
- Save states
- Live core options
- Emulator-handled cheats through RetroArch's cheat interface

## RetroPad Button Mapping

| RetroPad Button | Native32 Keycode | Action |
|---|---|---|
| D-Pad Left | `0x0200` | Left |
| D-Pad Right | `0x0400` | Right |
| D-Pad Up | `0x1c00` | Up |
| D-Pad Down | `0x1e00` | Down |
| A (SNES East) | `0x8800` | B / Menu |
| B (SNES South) | `0x4000` | A |
| Select (Right Shift) | — | Return to the ZIP menu / exit |

### Return and Exit Behavior

RetroPad **Select** is the core's back action. When a `.zip` package has
launched one of its games, pressing Select returns to the package's `FHUI.smf`
menu. Pressing Select from that menu, or while running a directly loaded
`.smf`, `.sgm`, or `.ssl` file, asks the frontend to close the core.

Select is handled on the initial press only and is not sent to Native32 game
input. If Select is also configured as a RetroArch hotkey, remap either the
core action or the conflicting frontend hotkey.

## Core Options

Options are available from **Quick Menu > Core Options**. Changes apply live
without reloading the game.

| Option | Values | Default | Effect |
|---|---|---|---|
| Audio Volume (%) | 0-100 (steps of 10) | 100 | Output volume; 0 mutes |
| Key auto-repeat delay (frames) | 2-30 | 12 | Frames a held key waits before auto-repeat starts (~0.4s at 30fps) |
| Key auto-repeat period (frames) | 1-10 | 3 | Frames between auto-repeat pulses (~0.1s at 30fps) |
| Swap A/B buttons | disabled / enabled | disabled | Swaps the A and B button actions |
| Auto-skip cutscene videos | disabled / enabled | disabled | Skips logo, intro, and cutscene videos automatically |

The auto-repeat options reproduce the original keypad's typematic behavior,
which some games use to distinguish walking from running while a direction is
held.

## Cheats

Native32Emu uses RetroArch's **Emulator** cheat handler. Native32Emu rules are
sent to the core through the libretro cheat interface; they are not RetroArch
memory-address cheats. The code strings use the same syntax as the standalone
emulator:

- `var:<name>=<value>` - force a Native32 VM variable.
- `sprite:<name>.<field>=<value>` or `movie:<name>.<field>=<value>` - force a
  movie sprite field. Supported fields are `x`, `y`, `depth`, `frame`,
  `visible`, and `playing`.
- `frame:goto=<n>` - force the main timeline to jump to a frame.
- `frame:playing=<bool>` - force the main timeline play/pause state.

Boolean values accept `1`/`0`, `true`/`false`, `on`/`off`, and `yes`/`no`.

1. Load a game and open **Quick Menu > Cheats**.
2. Select **Add New Code to Top** or **Add New Code to Bottom**.
3. Open the new entry and set **Handler** to **Emulator**.
4. Enter a Native32Emu rule in **Code**, such as `var:p_hp=96`.
5. Set **Enabled** to **On**.
6. Return to the Cheats menu and select **Apply Changes**.

Use the standalone emulator's cheat debug options when you need to discover
variable and sprite names. Verified game-specific rules are listed in
[Game Cheat Codes](Cheat-Codes.md).

See the official
[RetroArch cheat code guide](https://docs.libretro.com/guides/cheat-codes/)
for more information about emulator-handled and RetroArch-handled cheats.
