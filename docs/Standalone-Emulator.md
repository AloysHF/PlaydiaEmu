# Standalone Emulator

This guide covers building and running the standalone `playdia-emu` binary,
loading discs, keyboard controls, headless mode, frame dumps, and all
command-line options.

## Supported Platforms

Release CI builds the standalone `playdia-emu` binary for the targets below
(see [Release Process](Release-Process.md)). Android / iOS / webOS receive
libretro cores only, not standalone binaries.

| Platform | Architecture | Status |
|----------|-------------|--------|
| Windows | x86_64 | Primary development; built in release CI |
| Linux | x86_64 | Built in release CI |
| Linux | aarch64 | Built in release CI |
| macOS | x86_64 | Built in release CI |
| macOS | aarch64 | Built in release CI |
| Android / iOS / webOS | — | Libretro only (no standalone binary) |

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
HLE player opens. `--headless` runs the HLE disc player without a window.
`-S/--screenshot` always runs headless for `--screenshot-frames` and exits.

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

Example with a local disc path (do not commit discs):

```powershell
cargo run --release -p playdiaemu -- `
  "path\to\game.cue" `
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
