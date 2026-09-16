# PlaydiaEmu — A Bandai Playdia emulator written in Rust

<p align="center">
  <img src="res/logo-banner.png" alt="PlaydiaEmu" width="600">
</p>

<p align="center">
  <a href="https://aloysHF.github.io/PlaydiaEmu/"><img src="https://img.shields.io/badge/Website-PlaydiaEmu-E8553A?logo=githubpages&logoColor=white" alt="Website"></a>
  <a href="https://github.com/AloysHF/PlaydiaEmu/actions/workflows/ci.yml"><img src="https://github.com/AloysHF/PlaydiaEmu/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://git.libretro.com/libretro/playdiaemu/-/pipelines"><img src="https://img.shields.io/gitlab/pipeline-status/playdiaemu?gitlab_url=https%3A%2F%2Fgit.libretro.com%2Flibretro&branch=master&logo=gitlab&label=Pipeline%20Status" alt="Gitlab Pipeline Status" ></a>
  <a href="https://github.com/AloysHF/PlaydiaEmu/releases/latest"><img src="https://img.shields.io/github/v/release/AloysHF/PlaydiaEmu" alt="Release"></a>
  <a href="https://github.com/AloysHF/PlaydiaEmu/releases"><img src="https://img.shields.io/github/downloads/AloysHF/PlaydiaEmu/total" alt="Downloads"></a>
  <a href="https://sonarcloud.io/dashboard?id=AloysHF_PlaydiaEmu"><img src="https://sonarcloud.io/api/project_badges/measure?project=AloysHF_PlaydiaEmu&metric=alert_status" alt="Quality Gate Status"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-BSD%203--Clause-blue.svg" alt="License: BSD 3-Clause"></a>
  <a href="https://discord.gg/7XDdSrYD"><img src="https://img.shields.io/badge/Discord-Join%20Us-5865F2?logo=discord&logoColor=white" alt="Discord"></a>
  <a href="https://qm.qq.com/q/LAO7DKAWUC"><img src="https://img.shields.io/badge/QQ%E7%BE%A4-Join%20Us-12B7F5?logo=tencent-qq&logoColor=white" alt="QQ Group"></a>
</p>

Playdia is a 1990s Japanese interactive CD console from Bandai. Titles are
largely full-motion video driven by a CD-XA disc stream, with audio and video
decoded on a co-processor board. PlaydiaEmu is a Rust workspace that plays
real CDS-XA content through an HLE disc player (no BIOS required) and also
ships an SH-1 LLE shell for future firmware experiments.

## Status

Research-grade but usable for disc playback:

- **HLE disc player (recommended)** — dual-track MODE2 CUE/BIN streaming, F1/F2/F3 video markers, XA ADPCM audio
- **Interactive stream control** — finish embedded video before F2 jumps or button choices; unsupported command details remain under investigation
- **Video** — recovered AK8000 row/VLC decoding produces recognizable 248×216 game pictures in a 320×240 XRGB8888 framebuffer (eight bits per color channel); hardware pixel accuracy remains unverified
- **Audio** — Green Book CD-XA ADPCM, resampled to 44100 Hz stereo
- **SH-1 LLE shell** — interpreter subset + proven memory map; retail boot needs a user-supplied 512 KiB BIOS at `0xE0000000`
- **Headless machine** — deterministic `run_frame`, save states, diagnostics
- **Standalone CLI / window** — window / `--headless` HLE player
- **Libretro core** — RetroArch-compatible cdylib shell

## Features

- **CDS-XA Form 2 streaming** — 2352-byte Mode2 sectors, dual-track CUE/BIN
- **F1/F2/F3 routing** — video fragments including F2 overflow, F3 padding, and F2 scene navigation
- **XA ADPCM audio** — 4-bit ADPCM sound groups, 37800/18900 Hz → stereo 44100
- **Native game video** — 27 rows of 4×4 transform blocks, run/level coefficients, macroblock DC prediction and separate Y/C quantizers; invalid pictures preserve the previous frame
- **Save states** — content identity + CRC envelope; version 2 preserves RGB888 pixels (version 1 states are rejected)
- **Headless / inspect tooling** — sector and packet diagnostics without a window
- **RetroArch integration** — libretro core for frontend use
- **Cross-crate core** — platform-independent `playdiaemu-core` with no host I/O

## Usage

### Standalone Mode (HLE, no BIOS)

```powershell
cargo run --release -p playdiaemu -- path\to\game.cue --headless --frames 180
# Redump-style ZIP works too (CUE+BIN inside; no extract needed)
cargo run --release -p playdiaemu -- path\to\game.zip --headless --frames 180
```

For a reproducible button choice in headless playback, add for example
`--press-at 120:a` (zero-based host frame; repeat the option for more presses).

Windowed playback:

```powershell
cargo run --release -p playdiaemu -- path\to\game.cue
cargo run --release -p playdiaemu -- path\to\game.zip
```

See the [Standalone Emulator](docs/Standalone-Emulator.md) guide for
installation, keyboard controls, headless mode, screenshots/PPM dumps, and all
command-line options. Batch title-screen captures live in
`scripts/batch-screenshots.ps1`; the published matrix is
[Game Compatibility](docs/Game-Compatibility.md).

### RetroArch Mode

Build the libretro core and load a disc image through RetroArch's
**Load Content** menu:

```powershell
cargo build --release -p playdiaemu-libretro
```

The libretro core uses the same HLE disc player as the standalone emulator
(dual-track MODE2 CUE/BIN, F1/F2/F3, XA audio; no BIOS). See the
[RetroArch Core](docs/RetroArch-Core.md) guide for installation, RetroPad
mapping, supported features, and current limitations.

## Building

Requires [Rust](https://www.rust-lang.org/tools/install) (stable).

### Standalone Mode

```powershell
cargo build --release -p playdiaemu
cargo run --release -p playdiaemu -- path\to\game.cue --headless --frames 180
```

### Window frontend

```powershell
cargo build --release -p playdiaemu
```

The binary is produced at `target\release\playdia-emu.exe` (`.exe` on Windows).

### Libretro Core (for RetroArch)

```powershell
cargo build --release -p playdiaemu-libretro
```

Cargo names the cdylib after its lib target, so this produces
`playdiaemu.dll` on Windows (`libplaydiaemu.so` on Linux,
`libplaydiaemu.dylib` on macOS) under `target/release/`. Rename it to
`playdiaemu_libretro.<ext>` before placing it in RetroArch's `cores/`
directory. Copy `playdiaemu_libretro.info` into RetroArch's `info/`
directory.

### Tools

```powershell
cargo run --release -p playdiaemu-tools --bin playdia-inspect -- path\to\game.cue
```

For video packet header frequencies, run
`cargo run --release -p playdiaemu-tools --bin playdia-inspect -- path\to\game.cue --video-headers`.
Add `--video-candidates` to rank packets by low body-byte entropy and long
`0x55`/`0xAA` runs. The reported track-relative LBAs help target codec research;
these patterns do not establish decoded pixels or a VLC table.
Use `--video-rows` to count MSB-first 27-row sequences, ambiguous marker matches,
and picture terminators in F2 tails. Packet assembly preserves pending video
across FF-filled F3 sectors. See [AK8000 research](docs/AK8000-Research.md) for
evidence, the recovered decoder and remaining pixel-accuracy questions.
`python tools/probe_sparse_vlc.py path\to\track.bin` checks a conservative partial
codeword grammar on short rows; it also accepts a ZIP containing Track 2.
Its counts are research diagnostics, not decoded coefficients or game pixels.

Decode a specific picture without navigating the game, or validate a whole disc:

```powershell
cargo run --release -p playdiaemu-tools --bin playdia-frame -- game.cue --packet 523 --output scene.ppm
cargo run --release -p playdiaemu-tools --bin playdia-frame -- game.cue --check-all
```

Packet numbers start at 1 and include interactive F2 packets. The frame tool
exports 248×216 RGB888 PPMs; the player exports the same RGB888 colors
centered in 320×240. Neither path reduces channels to five bits. A [37-disc validation run](docs/AK8000-Corpus-Validation.md) passed
1,135,531 of 1,135,539 picture packets; eight packets end inside their final row.
This measures entropy coverage, not full game compatibility or hardware pixel accuracy.

## Testing

Run the unit tests:

```powershell
cargo test --workspace
```

CI runs formatting, clippy, tests, and a release build on every push and pull
request (see `.github/workflows/ci.yml`). Prefer `--release` for any real-disc
decode or visual check; debug builds are for unit tests and fast iteration only.

## Architecture

```
crates/
├── playdiaemu-core/            # Platform-independent emulator engine (library)
│   └── src/
│       ├── lib.rs           # Crate root
│       ├── machine.rs       # LLE machine (SH-1 + bus + CDXA device)
│       ├── player.rs        # HLE DiscPlayer (Track 2 stream)
│       ├── sh1.rs           # SH-1 interpreter subset
│       ├── bus.rs           # Address space + diagnostics
│       ├── cd.rs            # Disc image / CUE/BIN loading
│       ├── cdx.rs           # CDS-XA demux and routing
│       ├── audio.rs         # XA ADPCM decode + resample
│       ├── video.rs         # F1 packet / DCT path
│       ├── bitstream.rs     # Bit reader
│       ├── ac_tables.rs     # Coefficient / VLC tables
│       ├── input.rs         # Button state
│       ├── state.rs         # Save-state envelope
│       ├── content.rs       # Content identity
│       └── diagnostics.rs   # Unmapped / unknown / budget counters
├── playdiaemu/              # Standalone binary (→ playdia-emu)
│   └── src/
│       └── main.rs          # Window + CLI (window / --headless)
├── playdiaemu-libretro/        # libretro cdylib (→ playdiaemu_libretro.{dll,so,dylib})
│   ├── playdiaemu_libretro.info
│   └── src/lib.rs           # libretro C ABI
└── playdiaemu-tools/           # ISO/XA inspector and research utilities
    └── src/bin/
        ├── playdia_inspect.rs
        └── playdia_scan.rs
```

### Core contract

- `load_disc_*` / optional `load_bios_*`
- `reset`, `run_frame`, `set_input`
- `framebuffer` → 320×240 XRGB8888 (`u32`, `0x00RRGGBB`)
- `drain_audio` → interleaved stereo i16 @ 44100
- `save_state` / `load_state` with content identity + CRC
- diagnostics for unmapped access and unknown opcodes

For the proven memory map, CDS-XA routing, and codec notes, see
[Game File Formats](docs/Game-File-Formats.md).

## Key Mappings (Standalone window)

| Key | Button |
|-----|--------|
| Arrow Up/Down/Left/Right or W/A/S/D | D-pad |
| Z or J | A |
| X or K | B |
| Enter | Start |
| Space | Select |
| Escape | Exit |

## Game Compatibility

HLE player (no BIOS). A small set of private Redump samples has been verified
for load, stream routing, video frames, and audio PCM.

| Title | Status |
|-------|--------|
| Mari-nee no Heya | Native title/button screen; HLE video/audio playback checked |
| Playdia Sample Soft | Native menu and demo imagery; HLE playback checked |
| Other Redump titles | See the 37-disc entropy report; complete playthroughs remain unverified |

For the detailed matrix and how to update it, see
[Game Compatibility](docs/Game-Compatibility.md).

## Content Formats

Most tested Playdia software uses dual-track CUE/BIN images with 2352-byte
Mode2 sectors. Track 1 is ISO9660 data; Track 2 carries the interactive FMV /
audio stream with F1/F2/F3 markers and XA ADPCM. Two tested titles use a single
MODE2/2352 track instead.

See [Game File Formats](docs/Game-File-Formats.md) for sector layout, Mode 2
subheader bits, Form 2 audio groups, and video markers.

## Contributing

Contributions are welcome — compatibility testing, codec research, SH-1
accuracy, docs, and bug reports. See [CONTRIBUTING.md](docs/CONTRIBUTING.md)
for details, code style, and the local CI checks.

## License

This project is licensed under the [BSD 3-Clause License](LICENSE).
