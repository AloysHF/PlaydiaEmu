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
3. Choose a `.cue` (preferred), Redump-style `.zip`, `.iso`, or raw `.bin` disc image.

> The core advertises `cue|zip|iso|bin` and requires the full content path
> (`need_fullpath`, `block_extract`). Content loading uses the same HLE
> `DiscPlayer` path as the standalone emulator: dual-track MODE2 CUE/BIN
> (also when the CUE/BIN pair is inside a ZIP), F1/F2/F3 routing, XA audio.
> No BIOS is required. ZIP paths are passed to the core so the frontend does
> not extract archives.

## Supported Features

- HLE disc player (`DiscPlayer`) aligned with the standalone emulator
- Video output using XRGB8888 (320×240, eight bits per channel, 1,280-byte row pitch)
- Stereo audio output at 44100 Hz
- RetroPad input handling (including interactive F2 choice mapping)
- Input descriptors for the frontend key-remap UI
- Performance level 4 (same as SPMP8000/Dingoo HLE shells)
- Rust `log` messages forwarded to the RetroArch log interface
- Host pacing at 30 fps (matches standalone HLE video slot rate)
- Save states via libretro serialize / unserialize (version 2 envelope; disc CRC must match)

## Core Options

| Option key | Values | Default |
|---|---|---|
| `playdiaemu_volume` | 100…0% | 100% |
| `playdiaemu_swap_ab` | disabled / enabled | disabled |
| `playdiaemu_debug_logging` | disabled / enabled | disabled |

Volume scales host PCM before submit. Swap A/B exchanges RetroPad A and B.
Debug logging raises the `log` crate level to Debug so more records reach
the RetroArch log.

## RetroPad Button Mapping

| RetroPad Button | Playdia Button |
|---|---|
| D-Pad Up / Down / Left / Right | Up / Down / Left / Right |
| A | A |
| B | B |
| Start | Start |
| Select | Select |
| X / Y | Unused |

At interactive F2 choice screens the same mapping selects destinations:
A/Start, B, Right, Left, Up, Down.

## Timing

| Field | Value |
|-------|-------|
| Base resolution | 320×240 |
| Aspect ratio | 4:3 |
| Frame rate | 30 fps (HLE host frames; matches standalone) |
| Sample rate | 44100 Hz |

## Limitations

- Cheats are stubbed (`retro_cheat_*` no-ops)
- No memory maps (HLE player has no fixed guest address space)
- No Android / iOS / webOS packaging docs yet

## Building notes

The core is a `cdylib` with no extra host dependencies beyond `playdia-core`:

```powershell
cargo build --release -p playdiaemu-libretro
```

## See also

- [Standalone Emulator](Standalone-Emulator.md)
- [Game File Formats](Game-File-Formats.md)
- [Game Compatibility](Game-Compatibility.md)
