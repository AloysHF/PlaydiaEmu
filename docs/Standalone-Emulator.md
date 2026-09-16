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
playdia-emu [OPTIONS] DISC
```

There are no subcommands. With a disc path and no `--headless`, the windowed
HLE player opens. `--headless` runs the HLE disc player without a window
(aligned with `spmp8000-emu` / `dingoo-emu`). `-S/--screenshot` always runs
headless for `--screenshot-frames` and exits. Disc inspection lives in
`playdiaemu-tools` (`playdia-inspect`).

## HLE disc player (recommended, no BIOS)

```powershell
cargo run --release -p playdiaemu -- path\to\game.cue --headless --frames 180
```

### HLE options

| Option | Default | Description |
|---|---|---|
| `<DISC>` | *required* | Path to `.cue` (preferred) or raw `.bin`/`.iso` |
| `--headless` | off | Run without opening a window |
| `--frames N` | `180` | Host frames to run in headless mode (~8 stream sectors each) |
| `-S, --screenshot PATH` | — | Take a screenshot after N frames and exit (PNG); implies headless |
| `--screenshot-frames N` | `30` | Frames before the screenshot (overrides `--frames` when `-S` is set) |
| `--dump-every N` | — | Periodic PPM dumps every N decoded frames |
| `--dump-dir DIR` | `tmp/out` | Directory for periodic dumps |
| `--press-at FRAME:BUTTON` | — | Inject a one-frame press; repeat for multiple inputs. Buttons: `up`, `down`, `left`, `right`, `a`, `b`, `start` |

The HLE player follows F2 scene jumps and pauses at F2 button choices until a
mapped button is pressed. CUE/BIN images provide the full-disc addresses needed
for these jumps. Timeout, score, and quiz behavior is still incomplete.
The default decoder reconstructs 248×216 game pictures centered in the 320×240
XRGB8888 framebuffer, retaining eight bits per color channel. It validates all 27 rows before presenting a picture. Unknown
codes or damaged packets retain the previous frame. Rare VLC entries and
hardware transform/color rounding still need validation.

Example: `playdia-emu game.cue --headless --frames 180 --press-at 122:a`.
Frame numbers start at zero. Use a frame after a choice prompt appears; the
`waiting=true` field in the progress output marks that state.

Screenshot-only (30 frames by default):

```powershell
playdia-emu game.cue -S preview.png --screenshot-frames 60
```

Example (private research corpus — do not commit discs):

```powershell
cargo run --release -p playdiaemu -- `
  "tmp/iso/Mari-nee no Heya (Japan)/Mari-nee no Heya (Japan).cue" `
  --headless --frames 90
```

## Window frontend

```powershell
cargo run --release -p playdiaemu -- path\to\game.cue
```

### Window options

| Option | Default | Description |
|---|---|---|
| `<DISC>` | *required* | Path to `.cue` (preferred) or raw `.bin`/`.iso` |
| `-s, --scale N` | `1` | Window scale factor (native 320×240, 1–8) |
| `-f, --fullscreen` | off | Borderless fullscreen |
| `--fps N` | `30` | Target FPS |
| `-v, --volume N` | `100` | Master audio volume (0–100; `0` disables audio) |
| `--remap BUTTON:KEY` | — | Remap a button (repeatable). Buttons: `up`, `down`, `left`, `right`, `a`, `b`, `start`, `select` |
| `--swap-ab` | off | Swap emulated A and B |
| `--no-gamepad` | off | Disable physical gamepad input (keyboard remains available) |
| `--show-gamepad` | off | Draw button-state overlay on the game frame |
| `--debug-logging` | off | Enable emulator debug logging (default filter `info`) |

Window mode runs until the window is closed (or Esc / end of disc). `--frames`
and `--screenshot` apply only to the headless path.

### Key mappings

| Key | Button |
|-----|--------|
| Arrow Up/Down/Left/Right | D-pad |
| Z | A |
| X | B |
| Enter | Start |
| Right Shift | Select |
| Escape | Exit |

Use `--remap` to replace a binding (for example `--remap a:space`). Escape is reserved for exit.

## Inspect

Disc inspection is provided by `playdiaemu-tools` (`playdia-inspect`). Print disc
kind, tracks, CRC, ISO volume label sample, and stream-track F1/F2/F3 / audio
sector counts:

```powershell
cargo run --release -p playdiaemu-tools --bin playdia-inspect -- path\to\game.cue
cargo run --release -p playdiaemu-tools --bin playdia-inspect -- path\to\game.cue --video-headers
cargo run --release -p playdiaemu-tools --bin playdia-inspect -- path\to\game.cue --video-rows
cargo run --release -p playdiaemu-tools --bin playdia-inspect -- path\to\game.cue --video-candidates
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
cargo run --release -p playdiaemu-tools --bin playdia-frame -- game.cue --packet 523 --output scene.ppm
cargo run --release -p playdiaemu-tools --bin playdia-frame -- game.cue --check-all
```

This tool reads pictures in disc order without following scene commands.
Indices start at 1 and include interactive F2 pictures. `--check-all` reports
failed packet indices, track-relative LBAs, rows, blocks and bit offsets, and
exits unsuccessfully if any picture fails. CUEs and raw MODE2/2352 BIN tracks
are supported. `--assembled` accepts an already assembled packet for isolated
debugging. PPM output is 248×216 RGB888. The player and its 320×240 PPM screenshots
preserve exactly the same channel precision, with a centered black border.

## Batch screenshots

`scripts/batch-screenshots.ps1` captures a PNG for every disc under a folder
(same pattern as `spmp8000-emu` / `dingoo-emu`). Output goes to
`docs/images/`. Redump-style ZIPs are loaded in place:

```powershell
# Rebuild release binary, then capture every .zip under -RedumpDir
.\scripts\batch-screenshots.ps1 -RedumpDir <local-redump-folder>

# Existing binary, extracted CUE/BIN tree under tmp\playdia_game
.\scripts\batch-screenshots.ps1 -SkipBuild -GameDir tmp\playdia_game
```

Optional: `-Frames` (default 300, with a few title-screen overrides that may
also inject `--press-at` to leave the BANDAI boot logo), `-TimeoutSeconds`
(default 300), `-Binary`, `-SkipBuild`. Do not commit disc images; only the
PNG previews and the script belong in the repository. See
[Game Compatibility](Game-Compatibility.md) for the published matrix.

## Audio Output

XA ADPCM is decoded and resampled to 44100 Hz stereo. The window frontend
plays audio through the default host sink; use `-v 0` to disable it.

## Logging

Both binaries use `env_logger`. Default filter is `info`. Pass
`--debug-logging` to use `debug` instead, or set `RUST_LOG` to override:

```powershell
playdia-emu game.cue --debug-logging
$env:RUST_LOG="debug"
cargo run --release -p playdiaemu -- path\to\game.cue --headless --frames 30
```

## See also

- [Game File Formats](Game-File-Formats.md)
- [Game Compatibility](Game-Compatibility.md)
- [RetroArch Core](RetroArch-Core.md)
