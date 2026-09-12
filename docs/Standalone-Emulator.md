# Standalone Emulator

This guide covers building and running the standalone Playdia binaries, loading
discs, keyboard controls, headless mode, frame dumps, and all command-line
options.

## Supported Platforms

| Platform | Architecture | Status |
|----------|-------------|--------|
| Windows | x86_64 | Primary development |
| Linux | x86_64 | Expected to work (CI on ubuntu-latest) |
| macOS | x86_64, aarch64 | Expected to work |

## Installation

Build from source:

```powershell
# CLI (play / inspect / headless)
cargo build --release -p playdia

# Window frontend
cargo build --release -p playdia --bin playdia-emu
```

Binaries:

- `target/release/playdia.exe` — CLI
- `target/release/playdia-emu.exe` — window frontend (minifb + rodio)

## Synopsis

### CLI

```text
playdia <COMMAND>

Commands:
  inspect   Inspect a CUE/BIN or raw disc image
  play      HLE disc player: stream Track 2 video/audio without BIOS
  headless  LLE-oriented headless (SH-1 + bus)
```

### Window

```text
playdia-emu [OPTIONS] <DISC>
```

## HLE disc player (recommended, no BIOS)

```powershell
cargo run --release -p playdia -- play path\to\game.cue --frames 180 --dump-ppm out.ppm
```

### `play` options

| Option | Default | Description |
|---|---|---|
| `<DISC>` | *required* | Path to `.cue` (preferred) or raw `.bin`/`.iso` |
| `--frames N` | `180` | Host frames to run (~8 stream sectors each) |
| `--dump-ppm PATH` | — | Write final 320×240 PPM |
| `--dump-every N` | — | Periodic PPM dumps every N frames |
| `--dump-dir DIR` | `tmp/out` | Directory for periodic dumps |
| `--full-decode` | off | Experimental AC quantization scaling (default uses raw coefficients) |
| `--press-at FRAME:BUTTON` | — | Inject a one-frame press; repeat for multiple inputs. Buttons: `up`, `down`, `left`, `right`, `a`, `b`, `start` |

The HLE player follows F2 scene jumps and pauses at F2 button choices until a
mapped button is pressed. CUE/BIN images provide the full-disc addresses needed
for these jumps. Timeout, score, and quiz behavior is still incomplete.
The current video decoder renders approximate blocks; it cannot reproduce the
original game picture until the AK8000 entropy format is recovered.

Example: `playdia play game.cue --frames 180 --press-at 122:a --dump-ppm scene.ppm`.
Frame numbers start at zero. Use a frame after a choice prompt appears; the
`waiting=true` field in the progress output marks that state.

Example (private research corpus — do not commit discs):

```powershell
cargo run --release -p playdia -- play `
  "tmp/iso/Mari-nee no Heya (Japan)/Mari-nee no Heya (Japan).cue" `
  --frames 90 --dump-ppm tmp/out/mari.ppm
```

## Window frontend

```powershell
cargo run --release -p playdia --bin playdia-emu -- path\to\game.cue
```

### `playdia-emu` options

| Option | Default | Description |
|---|---|---|
| `<DISC>` | *required* | Path to `.cue` (preferred) or raw `.bin`/`.iso` |
| `--scale N` | `3` | Window scale factor (native 320×240, clamp 1–8) |
| `--fps N` | `30` | Target FPS |
| `--full-decode` | off | Experimental AC quantization scaling |
| `--mute` | off | Mute host audio |
| `--frames N` | `0` | Quit after N host frames (`0` = until window closed) |
| `--dump-ppm PATH` | — | Save PPM when quitting via `--frames` |

### Key mappings

| Key | Button |
|-----|--------|
| Arrow Up/Down/Left/Right or W/A/S/D | D-pad |
| Z or J | A |
| X or K | B |
| Enter | Start |
| Space | Select |
| Escape | Exit |

## Inspect

Print disc kind, tracks, CRC, ISO volume label sample, and stream-track
F1/F2/F3 / audio sector counts:

```powershell
cargo run --release -p playdia -- inspect path\to\game.cue
cargo run --release -p playdia-tools -- path\to\game.cue
cargo run --release -p playdia-tools --bin playdia-inspect -- path\to\game.cue --video-headers
```

The last command assembles F1/F2 video packets and reports header validation,
secondary segment-code frequencies, and packet lengths without exporting video
data.

## LLE headless (optional)

Requires a user-supplied BIOS for retail boot. Prefer `play` for disc playback.

```powershell
cargo run --release -p playdia -- headless path\to\disc.iso --bios bios.bin --frames 60
```

### `headless` options

| Option | Default | Description |
|---|---|---|
| `<DISC>` | *required* | Disc image path |
| `--bios PATH` | — | 512 KiB BIOS EPROM |
| `--frames N` | `60` | Frames to run |
| `--allow-placeholder-bios` | off | Explicit test mode (refuses silent zero-fill otherwise) |
| `--audio-test-tone` | off | Emit a test tone instead of disc audio |
| `--save-state PATH` | — | Write save state after the run |
| `--load-state PATH` | — | Load save state before the run |
| `--dump-fb PATH` | — | Dump framebuffer |

## Audio Output

XA ADPCM is decoded and resampled to 44100 Hz stereo. The window frontend
plays audio through the default host sink; use `--mute` to disable it.

## Logging

Both binaries use `env_logger`. Default filter is `info`. For example:

```powershell
$env:RUST_LOG="debug"
cargo run --release -p playdia -- play path\to\game.cue --frames 30
```

## See also

- [Game File Formats](Game-File-Formats.md)
- [Game Compatibility](Game-Compatibility.md)
- [RetroArch Core](RetroArch-Core.md)
