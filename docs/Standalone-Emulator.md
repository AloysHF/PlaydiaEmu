# Standalone emulator

## Build

```powershell
cargo build --release -p playdia
```

## Inspect a disc

```powershell
cargo run --release -p playdia -- inspect path\to\disc.iso
# or
cargo run --release -p playdia-tools -- path\to\disc.iso
```

## Headless run

```powershell
cargo run --release -p playdia -- headless path\to\disc.iso `
  --bios path\to\playdia_bios.bin `
  --frames 180
```

Without a BIOS dump (streaming experiments only):

```powershell
cargo run --release -p playdia -- headless path\to\disc.iso `
  --allow-placeholder-bios --frames 60 --audio-test-tone
```

Options:

| Flag | Meaning |
|------|---------|
| `--bios` | 512 KiB SH-1 EPROM at `0xE0000000` |
| `--allow-placeholder-bios` | empty EPROM + RAM idle loop (tests) |
| `--frames N` | stop after N video frames |
| `--save-state PATH` | write save-state after run |
| `--load-state PATH` | load save-state before run |
| `--dump-fb PATH` | write 320×240 RGB555 LE framebuffer |
| `--audio-test-tone` | ignore ADPCM, emit sine |

## Windowed frontend

Not yet implemented in the standalone binary. The libretro core
(`target/release/playdia_libretro.dll`) can be loaded by RetroArch-compatible
frontends for interactive presentation.
