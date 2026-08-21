# iOS Libretro Core

Native32Emu runs as a libretro core on RetroArch, allowing you to play Native32 games on iPhone / iPad.

> **Note**: iOS does not currently support downloading cores via RetroArch's Online Updater. You need to manually inject the core into the RetroArch IPA. This limitation may be resolved in future RetroArch releases.

## Install in RetroArch on iOS

### Prerequisites

- iPhone or iPad (arm64, iOS 15+)
- RetroArch 1.17.0 IPA ([official download](https://buildbot.libretro.com/stable/1.17.0/apple/ios-arm64/RetroArch.ipa))
  - Version 1.17.0 is recommended; newer versions have a different folder structure that makes manual injection more complex
- Download `native32-emu-ios-libretro.tar.gz` from the [Releases](https://github.com/AloysHF/Native32Emu/releases) page. It contains:
  - `native32emu_libretro_ios.dylib` — core binary (real devices: arm64 + x86_64 universal)
  - `native32emu_libretro.info` — core metadata
- A file manager and IPA signing app (e.g. ESign, SideStore, or AltStore)

### Step 1: Rename IPA to ZIP and extract

1. Rename the downloaded `RetroArch.ipa` to `RetroArch.zip`
2. Extract the ZIP file to get the `Payload/` folder
3. Navigate into `Payload/RetroArch.app/`

### Step 2: Inject the core binary

Copy `native32emu_libretro_ios.dylib` into: `Payload/RetroArch.app/modules/`

### Step 3: Inject the core metadata

1. Locate `assets.zip` inside `RetroArch.app/`
2. Extract `assets.zip` and navigate into the extracted `assets/` directory
3. Copy `native32emu_libretro.info` into `assets/info/`
4. Recompress the entire `assets/` directory, making sure the output is named `assets.zip`
5. Move the new `assets.zip` back into `RetroArch.app/`, replacing the original

### Step 4: Re-sign the IPA

This is the most critical step on iOS. The modified IPA must be re-signed before it can be installed.

**Using ESign on an iOS device:**

1. Install [ESign](https://www.e-sign.cn/) (or another signing tool)
2. Repackage the modified folder into an `.ipa` file
3. Import the IPA into ESign
4. Select "Sign" → choose your certificate (personal or developer)
5. Install after signing completes

**Using SideStore or AltStore on Mac/PC:**

1. Install SideStore or AltStore on your iOS device
2. Install the modified IPA through SideStore/AltStore

### Step 5: Run Online Updater and restart

1. Open RetroArch
2. Go to **Main Menu → Online Updater**
3. Run the following updates:
   - Update Core Info Files
   - Update Assets
   - Update Controller Profiles
   - Update Databases
   - Update Overlays
4. Restart RetroArch

The Native32 core should now appear in the core list automatically.

### Using a pre-built IPA (quick start)

If you prefer not to modify the IPA yourself, `celiocasttro` has provided RetroArch 1.17.0 IPAs with the Native32 core pre-injected:

- **Native32Emu v1.1.0**: https://www.mediafire.com/file/k7zyijk7eo2pfjh/RetroArch_v1.17.0_26205.ipa/file
- **Native32Emu v1.2.0**: https://www.mediafire.com/file/k368op318p00vkq/native32-emu-ios_retroarch.ipa/file

Video tutorial: https://youtu.be/MDoS2ir9cnI

### Troubleshooting

#### Core does not appear in RetroArch

- Make sure you are using RetroArch **1.17.0** (newer versions have a different folder structure)
- Verify `native32emu_libretro_ios.dylib` is in the `modules/` directory
- Verify `native32emu_libretro.info` is in `assets/info/` (inside `assets.zip`)
- Ensure the IPA was correctly re-signed

#### IPA installation fails

- iOS does not allow installing unsigned IPA files directly
- You must re-sign using ESign, SideStore, or AltStore

## Building the iOS core locally

Building for iOS requires [Rust](https://www.rust-lang.org/tools/install) (stable) on macOS, with iOS targets added:

```bash
rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim
```

Build for real devices (arm64 + x86_64 universal library):

```bash
# Build for arm64 (real devices)
cargo build -p native32emu-libretro --release --target aarch64-apple-ios

# Build for x86_64 (real devices)
cargo build -p native32emu-libretro --release --target x86_64-apple-ios

# Create universal library
lipo -create \
  target/aarch64-apple-ios/release/libnative32emu.dylib \
  target/x86_64-apple-ios/release/libnative32emu.dylib \
  -output native32emu_libretro_ios.dylib
```

Build for simulator (Apple Silicon Mac):

```bash
cargo build -p native32emu-libretro --release --target aarch64-apple-ios-sim
```

> Cargo names the cdylib `libnative32emu.dylib`; the CI release workflow packages it as `native32emu_libretro_ios.dylib` along with `native32emu_libretro.info`.

## Acknowledgments

Special thanks to **celiocasttro** for the invaluable contributions to iOS support: testing the core on real devices, developing the IPA injection method with detailed step-by-step instructions and video tutorials, providing a pre-built IPA for the community, and verifying audio fixes across multiple builds.

If you run into issues installing this core on iOS, the following threads may help:

- [#58](https://github.com/AloysHF/Native32Emu/issues/58)
- [#71](https://github.com/AloysHF/Native32Emu/issues/71)
- [#78](https://github.com/AloysHF/Native32Emu/issues/78)
