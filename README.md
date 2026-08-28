# Native32 Emulator —— A Native32 game emulator written in Rust

<p align="center">
  <img src="res/logo-banner.svg" alt="Native32 Emulator" width="600">
</p>

<p align="center">
  <a href="https://aloyshf.github.io/Native32Emu/"><img src="https://img.shields.io/badge/Website-Native32Emu-E8553A?logo=githubpages&logoColor=white" alt="Website"></a>
  <a href="https://github.com/AloysHF/Native32Emu/actions/workflows/ci.yml"><img src="https://github.com/AloysHF/Native32Emu/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://git.libretro.com/libretro/native32emu/-/pipelines"><img src="https://img.shields.io/gitlab/pipeline-status/native32emu?gitlab_url=https%3A%2F%2Fgit.libretro.com%2Flibretro&branch=master&logo=gitlab&label=Pipeline%20Status" alt="Gitlab Pipeline Status" ></a>
  <a href="https://github.com/AloysHF/Native32Emu/releases/latest"><img src="https://img.shields.io/github/v/release/AloysHF/Native32Emu" alt="Release"></a>
  <a href="https://github.com/AloysHF/Native32Emu/releases"><img src="https://img.shields.io/github/downloads/AloysHF/Native32Emu/total" alt="Downloads"></a>
  <a href="https://sonarcloud.io/dashboard?id=AloysHF_Native32Emu"><img src="https://sonarcloud.io/api/project_badges/measure?project=AloysHF_Native32Emu&metric=alert_status" alt="Quality Gate Status"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-BSD%203--Clause-blue.svg" alt="License: BSD 3-Clause"></a>
  <a href="https://discord.gg/7XDdSrYD"><img src="https://img.shields.io/badge/Discord-Join%20Us-5865F2?logo=discord&logoColor=white" alt="Discord"></a>
  <a href="https://qm.qq.com/q/LAO7DKAWUC"><img src="https://img.shields.io/badge/QQ%E7%BE%A4-Join%20Us-12B7F5?logo=tencent-qq&logoColor=white" alt="QQ Group"></a>
</p>

Native32 is a game format developed by Sunplus for DVD player and TV chipsets (circa 2005–2011). Games use `.smf`, `.sgm`, or `.ssl` file extensions and feature a stack-based, ActionScript-like virtual machine with raster graphics.

## Features

- **Full Native32 format support** — file loading, header decryption, resource table parsing
- **YUV & ARGB image decoding** — with packed-resource DEFLATE support, packbits/RLE decompression, and color space conversion
- **Action bytecode VM** — 36 opcodes covering arithmetic, logic, string ops, control flow, sprites, and I/O
- **Sprite/movie system** — animation, cloning, visibility control, depth-sorted rendering
- **Audio playback** — finite/infinite-loop MP3 music mixed with raw 16-bit PCM sound effects (11025Hz for YUV games, 22050Hz for ARGB games), output as stereo with mono sources duplicated across both channels
- **MPEG-1 cutscenes** — pure-Rust MPEG-1 video + MP2 audio decoder plays `SSL_PlayNext` logo/cutscene videos (no C dependency); skippable with A/B
- **ZIP archive support** — load game packages directly from `.zip` files (auto-extracts and loads `FHUI.smf` or `NA32UI.smf`)
- **Keyboard input** — configurable key remapping
- **Save system** — `.ssl_sav` file persistence
- **SSL multi-file content** — seamless switching between game levels/files
- **CLI controls** — scaling, fullscreen, volume adjustment
- **Cheat system** — modify VM variables, sprite properties, and frame state at runtime; includes debug logging to discover cheat targets
- **RetroArch integration** — libretro core for use with RetroArch frontend

## Usage

### Standalone Mode

Download the latest binary from the
[Releases](https://github.com/AloysHF/Native32Emu/releases) page and run:

```bash
native32-emu path/to/game.smf
```

See the [Standalone Emulator](docs/Standalone-Emulator.md) guide for
installation, ZIP menu behavior, keyboard controls, cheats, display settings,
and all command-line options.

### RetroArch Mode

Install **Native32 (Native32Emu)** from RetroArch's Core Downloader, or install
the release files manually, then load a supported game through **Load Content**.
Press RetroPad **Select** to return from a child game to the SMF that launched
it. Press it with no parent SMF to close the core. This works for directly
loaded menus as well as ZIP packages.

See the [RetroArch Core](docs/RetroArch-Core.md) guide for installation,
supported platforms and features, RetroPad mapping, core options, and cheats.

## Building

Requires [Rust](https://www.rust-lang.org/tools/install) (stable).

### Standalone Mode (Default)

```bash
cargo build -p native32emu --release
cargo run -p native32emu --release -- path/to/game.smf
cargo run -p native32emu --release -- -f path/to/game.smf
```

The binary is produced at `target/release/native32-emu`.

### Libretro Core (for RetroArch)

```bash
cargo build -p native32emu-libretro --release
```

Cargo names the cdylib after its lib target, so this produces `native32emu.dll`
on Windows (`libnative32emu.so` on Linux, `libnative32emu.dylib` on macOS) under
`target/release/`. RetroArch expects the core file to be named
`native32emu_libretro.<ext>`, so rename it accordingly before dropping it into
RetroArch's `cores/` directory.

For Android cross-compilation, see [Android Libretro Core](docs/Android-Libretro-Core.md).
For iOS, see [iOS Libretro Core](docs/iOS-Libretro-Core.md).

## Testing

Run the unit tests:

```bash
cargo test --workspace
```

There is also a smoke test that loads every available game, runs it for a number
of frames, and checks that the emulator neither panics nor produces a blank
frame. It needs the (non-distributed) game assets, so it is `#[ignore]`d by
default and run on demand:

```bash
# Uses <repo>/tmp/native32_game by default, or set NATIVE32_GAME_DIR
cargo test -p native32emu-core --test smoke -- --ignored --nocapture
```

## Architecture

```
crates/
├── native32emu-core/            # Platform-independent emulator engine (library)
│   └── src/
│       ├── lib.rs               # Crate root (module declarations)
│       ├── emulator.rs          # Shared Emulator + VmHost (both front-ends)
│       ├── actions.rs           # Action opcode enum (36 opcodes)
│       ├── action_vm.rs         # Stack-based virtual machine
│       ├── audio_engine.rs      # MP3/PCM audio (rodio for standalone, buffer for libretro)
│       ├── content_loader.rs    # SSL multi-file content switching
│       ├── dat_loader.rs        # .dat metadata / thumbnail decoder (front-end menu)
│       ├── des_constants.rs     # DES permutation tables and S-boxes
│       ├── error.rs             # Error types
│       ├── file_browser.rs      # FHUI front-end game-list directory enumeration
│       ├── file_loader.rs       # File I/O, header parsing, resource tables
│       ├── frame_player.rs      # Main timeline frame playback (30fps)
│       ├── header_decryptor.rs  # Custom DES ECB header decryption
│       ├── image_decoder.rs     # YUV 4:2:0 and ARGB1555 image decoders
│       ├── input_handler.rs     # Input to keycode mapping (keyboard / RetroPad)
│       ├── renderer.rs          # Frame rendering with depth sorting
│       ├── save_manager.rs      # Save data persistence (.ssl_sav)
│       └── sprite_system.rs     # Movie/sprite instance management
├── native32emu/                 # Standalone binary (-> native32-emu)
│   └── src/
│       ├── main.rs              # Window loop and thin front-end
│       └── standalone/
│           ├── cli.rs           # Command-line argument parsing
│           └── gamepad_overlay.rs  # On-screen virtual gamepad overlay
└── native32emu-libretro/        # libretro cdylib (-> native32emu_libretro.{dll,so,dylib})
    ├── native32emu_libretro.info   # RetroArch core metadata
    └── src/
        ├── lib.rs               # cdylib crate root
        └── libretro/
            ├── api.rs           # Exported libretro functions (retro_init, retro_run, etc.)
            ├── callbacks.rs     # Callback management for video/audio/input
            ├── constants.rs     # libretro constants
            ├── logger.rs        # Bridges the `log` crate to the libretro log interface
            └── types.rs         # libretro type definitions
```

## Game Compatibility

Game resources can be downloaded from [Baidu Netdisk](https://pan.baidu.com/s/1CuNeJe-RKXG_E-LhdI5ldg?pwd=aloy).

All 84 Native32 games in the test suite load and run without fatal errors.

| Category | Count | Status |
|----------|-------|--------|
| Main Menu | 1 | ✅ Pass |
| EACT (Action) | 11 | ✅ Pass |
| EELA (Educational) | 32 | ✅ Pass |
| EPOP (Hot/Featured) | 9 | ✅ Pass |
| EPUZ (Puzzle) | 24 | ✅ Pass |
| ESPG (Sport) | 3 | ✅ Pass |
| ETAB (Chess/Board) | 4 | ✅ Pass |
| **Total** | **84** | **✅ All Passed** |

For detailed game list with screenshots and descriptions, see [Game Compatibility](docs/Game-Compatibility.md).

## Contribute

Contributions are welcome! Whether you're interested in fixing bugs, adding features, improving documentation, or testing game compatibility, we'd love your help. See [CONTRIBUTING.md](docs/CONTRIBUTING.md) for details.

## Acknowledgments

- [n32emu](https://github.com/gatecat/n32emu) by Myrtle Shah — an earlier and simple Python implementation
- [BootlegGames Wiki](https://bootleggames.fandom.com/wiki/Native_32) — hardware documentation and game catalog
- [EVD Hardware Gameplay Recordings](https://www.youtube.com/watch?v=Y8LcdLGTNPQ&list=PLbIEtYwRsTQYFXR3dbWdDYSozlIQsRer3) — game recordings on actual EVD hardware

## License

This project is licensed under the [BSD 3-Clause License](LICENSE).
