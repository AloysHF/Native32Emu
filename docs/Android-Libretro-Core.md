# Android Libretro Core

The Native32Emu libretro core also runs on Android, so it can be reused by most
Android RetroArch-based frontends.

## Install in RetroArch on Android

### Via Online Updater (Recommended)

The easiest way is to download the core directly from RetroArch's built-in Online Updater:

1. Open RetroArch
2. Go to **Main Menu → Online Updater → Core Downloader**
3. Find and select **Native32 (Native32Emu)**, wait for the download to complete
4. Go back to **Main Menu → Load Core** — the Native32 core should appear

To update an installed core:

1. Open RetroArch
2. Go to **Main Menu → Online Updater → Update Installed Cores**

### Manual Installation (Alternative)

If the Online Updater is not available, you can install the core manually:

1. **Download** `native32-emu-android-libretro.tar.gz` from the
   [Releases](https://github.com/AloysHF/Native32Emu/releases) page. It
   contains `native32emu_libretro_android.so` for the `arm64-v8a`,
   `armeabi-v7a`, `x86` and `x86_64` ABIs.
2. **Install the core**: copy the `native32emu_libretro_android.so` matching
   your device's ABI (most modern devices are `arm64-v8a`) into RetroArch's
   `cores/` directory (typically
   `/storage/emulated/0/RetroArch/cores/` or the app's internal `cores/` path),
   and copy `native32emu_libretro.info` into RetroArch's `info/` directory.
3. **Load** the core and content the same way as on desktop.

## Building the Android core locally

Building for Android requires the [Android NDK](https://developer.android.com/ndk)
and [`cargo-ndk`](https://github.com/bbqsrc/cargo-ndk):

```bash
cargo install cargo-ndk
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
export ANDROID_NDK_HOME=/path/to/android-ndk

# Build all four ABIs (artifacts land in target/<triple>/release/)
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86 -t x86_64 -p 21 \
  build -p native32emu-libretro --release
```

Each ABI produces `libnative32emu.so`; rename it to
`native32emu_libretro_android.so` when installing into RetroArch on Android.
The CI release workflow performs this packaging automatically.

## RetroDroid

The same core ships inside [RetroDroid](https://github.com/AloysHF/RetroDroid),
the all-in-one Android app for this emulator family. RetroDroid loads the
`lib*.so` directly (no RetroArch required) and reads the neighbouring
`android.json` for the system profile: display names, content extensions,
virtual-controller preset, firmware requirements and orientation.

Rules the core follows for any Android host:

- Saves and in-game files go to the directory answered by
  `RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY`; the content path itself may be a
  disposable copy in the app cache.
- Load failures are reported with `RETRO_ENVIRONMENT_SET_MESSAGE` and the log,
  never as a silent `false`.
- Panics are caught at the C ABI boundary, so a bad frame cannot kill the host.
- Save states carry an `N32S` magic header plus a format version; states from
  another version are rejected instead of being misread.
- Cores are linked with 16 KB page alignment for Android 15 devices.
