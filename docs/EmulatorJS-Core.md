# EmulatorJS Core (Browser)

Native32Emu can run in the browser as a standard
[EmulatorJS](https://emulatorjs.org) core. The build follows EmulatorJS's own
pipeline: the Rust libretro core is compiled for `wasm32-unknown-emscripten`,
linked against the `EmulatorJS/RetroArch` fork, and packaged as the
`<core>-wasm.data` bundles that an EmulatorJS page loads at runtime.

## What you get

Every nightly and tagged release publishes:

| File | Purpose |
|------|---------|
| `native32-emu-emulatorjs.zip` | Complete drop-in bundle (recommended) |
| `native32emu-wasm.data` | WebGL2/WebAssembly core bundle |
| `native32emu-legacy-wasm.data` | WebGL1-compatible fallback bundle |

The core report (`native32emu.json`, build metadata) ships inside the ZIP as
`cores/reports/native32emu.json` rather than as a separate release asset.

The bundle is laid out like this:

```
native32-emu-emulatorjs.zip
├── cores/
│   ├── native32emu-wasm.data
│   ├── native32emu-legacy-wasm.data
│   └── reports/native32emu.json
├── core.json
├── build.json
├── license.txt
└── README.md
```

Each `.data` file is a 7z archive containing `<core>_libretro.js`,
`<core>_libretro.wasm`, `core.json`, `build.json` and `license.txt`.

## Install into a site

EmulatorJS always fetches cores from `<EJS_pathtodata>cores/`, so host the
runtime yourself (or use the pinned CDN files) and drop the core beside it:

```
data/
├── loader.js
├── emulator.min.js
├── emulator.min.css
├── compression/
│   ├── extract7z.js       # extracts the core .data archive
│   └── extractzip.js      # extracts ZIP game files
├── localization/          # optional, e.g. zh-CN.json
└── cores/
    ├── native32emu-wasm.data
    ├── native32emu-legacy-wasm.data
    └── reports/native32emu.json
```

`emulator.min.zip` only contains `emulator.min.js` and `emulator.min.css`;
the `compression/` and `localization/` files are loaded on demand from the
same EmulatorJS data directory. Without `extract7z.js` the core download ends
in a "Network Error".

To upgrade the runtime, replace the runtime files with a newer EmulatorJS
release (≥ 4.2.3) and keep this `cores/` directory.

## Use

```html
<div id="game"></div>
<script>
  EJS_player = "#game";
  EJS_core = "native32emu";
  EJS_gameUrl = "games/your-game.zip";
  EJS_pathtodata = "data/";
  EJS_startOnLoaded = false;
</script>
<script src="data/loader.js"></script>
```

`site/play.html` in this repository is a complete example: visitors pick a game
file from their own machine (or drop it on the page) and it runs from a blob
URL, so nothing is uploaded and the site does not need to host any game data.

### Content

The core accepts the same extensions as the desktop libretro build:
`.smf`, `.sgm` and `.zip`.

- A bare `.smf`/`.sgm` file is loaded directly.
- A `.zip` is extracted by EmulatorJS into the wasm filesystem with its
  directory structure preserved, then the core opens the archive's menu entry
  point (`FHUI.smf`, falling back to `NA32UI.smf`).
- Multi-file SSL games must be selected as a ZIP that keeps their original
  folders, for example:

  ```
  EACT/EBBLADE.smf
  NA32SSL/ENGLISH/BBLADE/*.SSL
  NA32SSL/ENGLISH/LOGO/NALOGO.MPG
  ```

  A bare `.smf` from such a game will boot, but the core cannot find its
  sibling SSL scenes and videos.

Game data is copyrighted and is neither distributed with this repository nor
required by the site: the play page only loads files the visitor selects.

## Saves

EmulatorJS mounts `/data/saves` as an IndexedDB-backed filesystem and points
RetroArch's `savefile_directory` there. The core queries that directory through
`RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY` and writes one `.ssl_sav` file per
content path, with path separators flattened to `_`
(e.g. `/EACT/EBBLADE.smf` → `EACT_EBBLADE.smf.ssl_sav`). Because the mount is
auto-persisted, progress survives page reloads in the same browser profile.

The core does not expose battery RAM through `retro_get_memory_data`, so
EmulatorJS's manual battery-save buttons are turned off by `"save": false` in
`core.json`. Save states are unaffected: they use `retro_serialize` and are
available from the EmulatorJS menu.

## Core options

The core registers the same options as the desktop build through the legacy
`SET_VARIABLES` interface, which RetroArch/EmulatorJS upgrades to its normal
options menu:

| Option | Default | Effect |
|--------|---------|--------|
| Audio Volume (%) | 100 | Output volume |
| Key auto-repeat delay (frames) | 12 | Direction key delay |
| Key auto-repeat period (frames) | 3 | Direction key repeat rate |
| Swap A/B buttons | disabled | Swap the two face buttons |
| Auto-skip cutscene videos | disabled | Skip MPEG logos/cutscenes automatically |

Values are re-read when the frontend signals a change
(`GET_VARIABLE_UPDATE`), so changes apply without reloading the game.

## Building locally

Requirements:

- an activated [emsdk](https://emscripten.org) 3.1.74 (`emmake` in `PATH`)
- the Rust `nightly` toolchain with the `rust-src` component
- the Rust target `wasm32-unknown-emscripten`
- `7z` (p7zip), `zip`, `git`

```bash
rustup toolchain install nightly --component rust-src
rustup +nightly target add wasm32-unknown-emscripten
source /path/to/emsdk/emsdk_env.sh          # Linux/macOS/WSL

cargo +nightly build -p native32emu-libretro \
  --target wasm32-unknown-emscripten --profile wasm-release \
  -Zbuild-std=std,panic_abort

scripts/build-emulatorjs-core.sh
```

`-Zbuild-std=std,panic_abort` rebuilds std with `panic=abort`. The prebuilt
emscripten stdlib contains wasm exception-handling instructions, and the
Asyncify pass that EmulatorJS's build forces on (for OpenAL audio) crashes on
them; the abort build emits none.

The build script:

1. fetches the pinned `EmulatorJS/RetroArch` commit
   (`86f72a39736b1fa351ca296d16f568e99e11a34f`) into `target/emulatorjs-build/`,
2. copies `target/wasm32-unknown-emscripten/wasm-release/libnative32emu.a` in as
   `native32emu_libretro_emscripten.bc`,
3. runs the fork's `build-emulatorjs.sh` twice (WebGL2 and `--legacy`),
4. writes `core.json`, `build.json`, `license.txt`, the core report and a
   README, and packages everything into `out/native32-emu-emulatorjs.zip`.

Useful flags:

```bash
scripts/build-emulatorjs-core.sh \
  --core path/to/libnative32emu.a \
  --out out/emulatorjs \
  --work-dir target/emulatorjs-build \
  --retroarch-ref <commit>
```

The wasm build needs a POSIX shell; on Windows run it inside WSL or an MSYS2
environment with emsdk activated, or rely on CI.

## CI integration

The `Nightly Build` and `Release` workflows run a `build-emulatorjs` job and
attach the artifacts to the rolling `nightly-build` / tagged releases. The
Pages workflow then:

1. downloads the pinned EmulatorJS 4.2.3 runtime into `site/data/`,
2. fetches `native32-emu-emulatorjs.zip` from `nightly-build` and copies the
   `.data` files and report into `site/data/cores/`.

No game data is deployed: `site/play.html` asks each visitor for a local file.

## Implementation notes

- `retro_get_system_info` advertises `smf|sgm|ssl|zip` with
  `need_fullpath = true` and `block_extract = true`, so EmulatorJS hands over
  the whole archive instead of a single entry.
- On Emscripten the core skips the libretro log interface (its variadic
  callback ABI does not survive the wasm bridge) and writes log messages to the
  browser console as `[Native32Emu] level: message`.
- The wasm profile uses `panic = "abort"` to avoid unwinding overhead.
- Local files are passed to EmulatorJS as blob URLs (File objects break its
  basename logic). `EJS_gameName` therefore keeps the original file name,
  including the extension, so single-file content is still recognised.
