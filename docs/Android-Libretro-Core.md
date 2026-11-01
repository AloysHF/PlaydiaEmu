# Android libretro core

The libretro crate builds as a `cdylib` named `libplaydiaemu.so`. The nightly
and release workflows cross-compile it for `arm64-v8a`, `armeabi-v7a`, `x86`
and `x86_64`, stage it as `playdiaemu_libretro_android.so`, and package it with
`playdiaemu_libretro.info` and `android.json`.

To build the same package locally:

```bash
export ANDROID_NDK_HOME=/path/to/ndk
export RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384"
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86 -t x86_64 --platform 21 \
  build -p playdiaemu-libretro --release
```

## RetroDroid

The same core ships inside [RetroDroid](https://github.com/AloysHF/RetroDroid),
the all-in-one Android app for this emulator family. RetroDroid loads the
`lib*.so` directly (no RetroArch required) and reads the neighbouring
`android.json` for the system profile: display names, content extensions,
virtual-controller preset, firmware requirements and orientation.

Rules the core follows for any Android host:

- Game saves and battery files belong in the directory answered by
  `RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY`; the content path itself may be a
  disposable copy in the app cache. PlaydiaEmu writes no battery files today
  (the HLE player keeps no memory card), so nothing is written next to content.
- Firmware belongs in the directory answered by
  `RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY`. The Playdia HLE player needs no
  BIOS, which is why `android.json` declares an empty firmware list.
- Load failures are reported with `RETRO_ENVIRONMENT_SET_MESSAGE` and the log,
  never as a silent `false`; CD package failures name the missing file or ZIP
  entry.
- Panics are caught at the C ABI boundary, so a bad frame cannot kill the host.
  The release profile therefore keeps `panic = "unwind"`; only the separate
  `wasm-release` profile opts into abort for Asyncify.
- Save states carry a `PDSS` magic header plus a little-endian format version;
  states from another version are rejected instead of being misread. The
  serialized core payload is stored verbatim after the header.
- Cores are linked with 16 KB page alignment for Android 15 devices.
