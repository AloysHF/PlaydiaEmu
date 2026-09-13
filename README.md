# PlaydiaEmu — A Bandai Playdia emulator written in Rust

Playdia is a 1990s Japanese interactive CD console from Bandai. Titles are
largely full-motion video driven by a CD-XA disc stream, with audio and video
decoded on a co-processor board. PlaydiaEmu is a Rust workspace that plays
real CDS-XA content through an HLE disc player (no BIOS required) and also
ships an SH-1 LLE shell for future firmware experiments.

## Status

Research-grade but usable for disc playback:

- **HLE disc player (recommended)** — dual-track MODE2 CUE/BIN streaming, F1/F2/F3 video markers, XA ADPCM audio
- **Interactive stream control** — finish embedded video before F2 jumps or button choices; unsupported command details remain under investigation
- **Video** — proprietary MPEG-1-like DCT path to 320×240 RGB555 (approximate until AK8000 VLC is locked)
- **Audio** — Green Book CD-XA ADPCM, resampled to 44100 Hz stereo
- **SH-1 LLE shell** — interpreter subset + proven memory map; retail boot needs a user-supplied 512 KiB BIOS at `0xE0000000`
- **Headless machine** — deterministic `run_frame`, save states, diagnostics
- **Standalone CLI / window** — `play` / `inspect` / `headless` plus `playdia-emu` window
- **Libretro core** — RetroArch-compatible cdylib shell

## Features

- **CDS-XA Form 2 streaming** — 2352-byte Mode2 sectors, dual-track CUE/BIN
- **F1/F2/F3 routing** — video fragments including F2 overflow, F3 padding, and F2 scene navigation
- **XA ADPCM audio** — 4-bit ADPCM sound groups, 37800/18900 Hz → stereo 44100
- **320×240 RGB555 framebuffer** — approximate fixed-count AC reconstruction by default; experimental `--full-decode` coefficient scaling. Neither path reproduces original game pixels yet
- **Save states** — content identity + CRC envelope
- **Headless / inspect tooling** — sector and packet diagnostics without a window
- **RetroArch integration** — libretro core for frontend use
- **Cross-crate core** — platform-independent `playdia-core` with no host I/O

## Usage

### Standalone Mode (HLE, no BIOS)

```powershell
cargo run --release -p playdiaemu -- play path\to\game.cue --frames 180 --dump-ppm out.ppm
```

For a reproducible button choice in headless playback, add for example
`--press-at 120:a` (zero-based host frame; repeat the option for more presses).

Windowed playback:

```powershell
cargo run --release -p playdiaemu -- path\to\game.cue
```

See the [Standalone Emulator](docs/Standalone-Emulator.md) guide for
installation, keyboard controls, headless mode, screenshots/PPM dumps, and all
command-line options.

### RetroArch Mode

Build the libretro core and load a disc image through RetroArch's
**Load Content** menu:

```powershell
cargo build --release -p playdiaemu-libretro
```

See the [RetroArch Core](docs/RetroArch-Core.md) guide for installation,
RetroPad mapping, supported features, and current limitations.

## Building

Requires [Rust](https://www.rust-lang.org/tools/install) (stable).

### Standalone Mode

```powershell
cargo build --release -p playdiaemu
cargo run --release -p playdiaemu -- play path\to\game.cue --frames 180
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
cargo run --release -p playdia-tools -- path\to\game.cue
```

For video packet header frequencies, run
`cargo run --release -p playdia-tools --bin playdia-inspect -- path\to\game.cue --video-headers`.
Add `--video-candidates` to rank packets by low body-byte entropy and long
`0x55`/`0xAA` runs. The reported track-relative LBAs help target codec research;
these patterns do not establish decoded pixels or a VLC table.
Use `--video-rows` to count MSB-first 26-row sequences, ambiguous marker matches,
and picture terminators in F2 tails. Packet assembly preserves pending video
across FF-filled F3 sectors. See [AK8000 research](docs/AK8000-Research.md) for
evidence and remaining decoding gaps; the preview still uses a speculative codec.
`python tools/probe_sparse_vlc.py path\to\track.bin` checks a conservative partial
codeword grammar on short rows; it also accepts a ZIP containing Track 2.
Its counts are research diagnostics, not decoded coefficients or game pixels.

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
├── playdia-core/            # Platform-independent emulator engine (library)
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
│       └── main.rs          # Window + CLI (play / inspect / headless)
├── playdiaemu-libretro/        # libretro cdylib (→ playdiaemu_libretro.{dll,so,dylib})
│   ├── playdiaemu_libretro.info
│   └── src/lib.rs           # libretro C ABI
└── playdia-tools/           # ISO/XA inspector and research utilities
    └── src/bin/
        ├── playdia_inspect.rs
        └── playdia_scan.rs
```

### Core contract

- `load_disc_*` / optional `load_bios_*`
- `reset`, `run_frame`, `set_input`
- `framebuffer` → 320×240 RGB555
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
| Mari-nee no Heya | Verified (approximate image) |
| Playdia Sample Soft | Verified (approximate image) |
| Other Redump titles | Expected same CUE layout; not yet listed |

For the detailed matrix and how to update it, see
[Game Compatibility](docs/Game-Compatibility.md).

## Content Formats

Playdia software ships as dual-track CD images (CUE/BIN, 2352-byte Mode2
sectors). Track 1 is ISO9660 data; Track 2 carries the interactive FMV /
audio stream with F1/F2/F3 markers and XA ADPCM.

See [Game File Formats](docs/Game-File-Formats.md) for sector layout, Mode 2
subheader bits, Form 2 audio groups, and video markers.

## Contributing

Contributions are welcome — compatibility testing, codec research, SH-1
accuracy, docs, and bug reports. See [CONTRIBUTING.md](docs/CONTRIBUTING.md)
for details, code style, and the local CI checks.

## Legal

Do not commit BIOS dumps, disc images, or extracted copyrighted assets.
`tmp/` is for private research materials only. Retail LLE boot requires a
user-supplied 512 KiB BIOS; the emulator refuses silent zero-fill outside
explicit test mode.

## License

This project is licensed under the [MIT OR Apache-2.0](Cargo.toml) dual license.
