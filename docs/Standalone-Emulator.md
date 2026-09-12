# Standalone emulator

## HLE disc player (recommended, no BIOS)

```powershell
cargo run --release -p playdia -- play path\to\game.cue --frames 180 --dump-ppm out.ppm
```

| Flag | Meaning |
|------|---------|
| `--frames N` | Host frames to run (~8 stream sectors each) |
| `--dump-ppm PATH` | Write final 320×240 PPM |
| `--dump-every N --dump-dir DIR` | Periodic PPM dumps |
| `--full-decode` | Experimental AC path (default is DC-only reconstruction) |

Example (private research corpus):

```powershell
cargo run --release -p playdia -- play `
  "tmp/iso/Mari-nee no Heya (Japan)/Mari-nee no Heya (Japan).cue" `
  --frames 90 --dump-ppm tmp/out/mari.ppm
```

## Inspect

```powershell
cargo run --release -p playdia -- inspect path\to\game.cue
cargo run --release -p playdia-tools -- path\to\game.cue
```

## LLE headless (optional)

```powershell
cargo run --release -p playdia -- headless path\to\disc.iso --bios bios.bin --frames 60
```

## Windowed UI

Not yet implemented. libretro core builds for frontend integration.
