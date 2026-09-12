# PlaydiaEmu

LLE-oriented emulator for the Bandai Playdia (SH7032 + CDXA).

## Status

Research-grade scaffold with:

- SH-1 interpreter subset
- Proven memory map
- CDS-XA Form2 audio (Green Book ADPCM) and F1/F2/F3 video routing
- Headless machine + save states
- Standalone CLI and libretro shell

Retail boot requires a user-supplied 512 KiB BIOS EPROM at `0xE0000000`.

## Build

```powershell
cargo test --workspace
cargo run -p playdia -- headless <disc.iso> --bios <bios.bin> --frames 180
cargo run -p playdia-tools -- <disc.iso>
```

## Legal

Do not commit BIOS dumps, disc images, or extracted copyrighted assets.
`tmp/` is for private research materials only.
