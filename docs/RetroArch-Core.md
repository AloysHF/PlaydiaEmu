# RetroArch Core

This guide covers installing and running the PlaydiaEmu libretro core for
RetroArch, loading content, supported features, controls, and current
limitations.

## Supported Platforms

| Platform | Architecture | Standalone | Libretro |
|----------|-------------|------------|----------|
| Windows | x86_64 | ✅ | ✅ (build from source) |
| Linux | x86_64, aarch64 | Expected | Expected |
| macOS | x86_64, aarch64 | Expected | Expected |
| Android | — | — | Built in CI (artifact only) |
| iOS | — | — | Built in CI (artifact only) |

## Installation

### Manual Installation

Build the libretro core:

```powershell
cargo build --release -p playdiaemu-libretro
```

Cargo names the cdylib after its lib target, so this produces
`playdiaemu.dll` on Windows (`libplaydiaemu.so` on Linux,
`libplaydiaemu.dylib` on macOS) under `target/release/`.

Rename it to `playdiaemu_libretro.<ext>` before placing it into RetroArch's
`cores/` directory. Copy `crates/playdiaemu-libretro/playdiaemu_libretro.info`
to RetroArch's `info/playdiaemu_libretro.info` so the frontend can display
core metadata.

## Loading Content

1. Open RetroArch and select **Load Core > Playdia (PlaydiaEmu)**.
2. Select **Load Content**.
3. Choose a `.cue`, `.iso`, or `.bin` disc image.

> The core advertises `cue|iso|bin` and requires the full content path
> (`need_fullpath`). Dual-track CUE/BIN support via the libretro front is
> limited compared with the standalone `play` path — prefer the standalone
> HLE player for real Redump CUE/BIN titles.

## Supported Features

- Video output using the 0RGB1555 pixel format (320×240)
- Stereo audio output at 44100 Hz
- RetroPad input handling
- Save states via libretro serialize / unserialize
- LLE machine path (`Machine`) rather than the standalone HLE `DiscPlayer`

## RetroPad Button Mapping

| RetroPad Button | Playdia Button |
|---|---|
| D-Pad Up / Down / Left / Right | Up / Down / Left / Right |
| A | A |
| B | B |
| Start | Start |
| Select | Select |
| X / Y | Unused |

## Timing

| Field | Value |
|-------|-------|
| Base resolution | 320×240 |
| Aspect ratio | 4:3 |
| Frame rate | 60 fps (core AV info) |
| Sample rate | 44100 Hz |

> Standalone HLE playback targets ~30 host frames/sec for disc video. The
> libretro AV info currently reports 60 fps while driving the LLE machine;
> treat this as a shell until content loading is fully aligned with the HLE
> player.

## Limitations

- No core options UI yet
- Cheats are stubbed (`retro_cheat_*` no-ops)
- No Android / iOS / webOS packaging docs yet
- Content loading is not yet the same dual-track HLE path as `playdia-emu play`
- BIOS is still required for retail LLE boot

## Building notes

The core is a `cdylib` with no extra host dependencies beyond `playdia-core`:

```powershell
cargo build --release -p playdiaemu-libretro
```

## See also

- [Standalone Emulator](Standalone-Emulator.md)
- [Game File Formats](Game-File-Formats.md)
- [Game Compatibility](Game-Compatibility.md)
