# Standalone Emulator

This guide covers building and running the standalone `playdia-emu` binary,
loading discs, keyboard controls, headless mode, frame dumps, and all
command-line options.

## Supported Platforms

| Platform | Architecture | Status |
|----------|-------------|--------|
| Windows | x86_64 | Primary development |
| Linux | x86_64 | Expected to work (CI on ubuntu-latest) |
| macOS | x86_64, aarch64 | Expected to work |

## Installation

Build from source:

```powershell
cargo build --release -p playdiaemu
```

The binary is produced at `target/release/playdia-emu.exe` (`.exe` on Windows).

## Synopsis

```text
playdia-emu [OPTIONS] [DISC] [COMMAND]

Commands:
  inspect   Inspect a CUE/BIN or raw disc image
  play      HLE disc player: stream Track 2 video/audio without BIOS
  headless  LLE-oriented headless (SH-1 + bus)
```

With no subcommand, `DISC` opens the windowed HLE player.

## HLE disc player (recommended, no BIOS)

```powershell
cargo run --release -p playdiaemu -- play path\to\game.cue --frames 180 --dump-ppm out.ppm
```

### `play` options

| Option | Default | Description |
|---|---|---|
| `<DISC>` | *required* | Path to `.cue` (preferred) or raw `.bin`/`.iso` |
| `--frames N` | `180` | Host frames to run (~8 stream sectors each) |
| `--dump-ppm PATH` | — | Write final 320×240 PPM |
| `--dump-every N` | — | Periodic PPM dumps every N frames |
| `--dump-dir DIR` | `tmp/out` | Directory for periodic dumps |
| `--full-decode` | off | Compatibility flag; native AK8000 decoding is already the default |
| `--press-at FRAME:BUTTON` | — | Inject a one-frame press; repeat for multiple inputs. Buttons: `up`, `down`, `left`, `right`, `a`, `b`, `start` |

The HLE player follows F2 scene jumps and pauses at F2 button choices until a
mapped button is pressed. CUE/BIN images provide the full-disc addresses needed
for these jumps. Timeout, score, and quiz behavior is still incomplete.
The default decoder reconstructs 248×216 game pictures centered in the 320×240
XRGB8888 framebuffer, retaining eight bits per color channel. It validates all 27 rows before presenting a picture. Unknown
codes or damaged packets retain the previous frame. Rare VLC entries and
hardware transform/color rounding still need validation.

Example: `playdia-emu play game.cue --frames 180 --press-at 122:a --dump-ppm scene.ppm`.
Frame numbers start at zero. Use a frame after a choice prompt appears; the
`waiting=true` field in the progress output marks that state.

Example (private research corpus — do not commit discs):

```powershell
cargo run --release -p playdiaemu -- play `
  "tmp/iso/Mari-nee no Heya (Japan)/Mari-nee no Heya (Japan).cue" `
  --frames 90 --dump-ppm tmp/out/mari.ppm
```

## Window frontend

```powershell
cargo run --release -p playdiaemu -- path\to\game.cue
```

### Window options

| Option | Default | Description |
|---|---|---|
| `<DISC>` | *required* | Path to `.cue` (preferred) or raw `.bin`/`.iso` |
| `--scale N` | `3` | Window scale factor (native 320×240, clamp 1–8) |
| `--fps N` | `30` | Target FPS |
| `--full-decode` | off | Compatibility flag; native decoding is already enabled |
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
cargo run --release -p playdiaemu -- inspect path\to\game.cue
cargo run --release -p playdia-tools --bin playdia-inspect -- path\to\game.cue
cargo run --release -p playdia-tools --bin playdia-inspect -- path\to\game.cue --video-headers
cargo run --release -p playdia-tools --bin playdia-inspect -- path\to\game.cue --video-rows
cargo run --release -p playdia-tools --bin playdia-inspect -- path\to\game.cue --video-candidates
```

The video commands assemble F1/F2 packets without exporting video data. The
candidate report also ranks low-entropy bodies and long `0x55`/`0xAA` runs,
including track-relative LBAs for follow-up codec analysis. These are encoded
bitstream patterns, not evidence of pixel-accurate decoding.
The row report counts ordered 27-row candidates and ambiguous matches. F2
overflow contributes actual video bytes; FF-filled F3 sectors preserve pending
video. See [AK8000 research](AK8000-Research.md) for the current evidence.
Interactive F2 sectors also finish pending video before a choice or jump.
The pure-Python `tools/probe_sparse_vlc.py` accepts a raw Track 2 BIN or its ZIP
and reports complete 186-block candidates, wrong counts and unresolved rows.
Its optional `--candidate-family` and `--gamma` rules remain unverified.

For native picture output and strict entropy validation:

```powershell
cargo run --release -p playdia-tools --bin playdia-frame -- game.cue --packet 523 --output scene.ppm
cargo run --release -p playdia-tools --bin playdia-frame -- game.cue --check-all
```

This tool reads pictures in disc order without following scene commands.
Indices start at 1 and include interactive F2 pictures. `--check-all` reports
failed packet indices, track-relative LBAs, rows, blocks and bit offsets, and
exits unsuccessfully if any picture fails. CUEs and raw MODE2/2352 BIN tracks
are supported. `--assembled` accepts an already assembled packet for isolated
debugging. PPM output is 248×216 RGB888. The player and its 320×240 PPM screenshots
preserve exactly the same channel precision, with a centered black border.

## LLE headless (optional)

Requires a user-supplied BIOS for retail boot. Prefer `play` for disc playback.

```powershell
cargo run --release -p playdiaemu -- headless path\to\disc.iso --bios bios.bin --frames 60
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
| `--dump-fb PATH` | — | Dump 320×240 little-endian XRGB8888 words (B, G, R, 0 bytes) |

Save states now use version 2 to preserve eight-bit color channels. Version 1
RGB555 states are rejected with an unsupported-version error; they are not
silently interpreted as the new format.

## Audio Output

XA ADPCM is decoded and resampled to 44100 Hz stereo. The window frontend
plays audio through the default host sink; use `--mute` to disable it.

## Logging

Both binaries use `env_logger`. Default filter is `info`. For example:

```powershell
$env:RUST_LOG="debug"
cargo run --release -p playdiaemu -- play path\to\game.cue --frames 30
```

## See also

- [Game File Formats](Game-File-Formats.md)
- [Game Compatibility](Game-Compatibility.md)
- [RetroArch Core](RetroArch-Core.md)
