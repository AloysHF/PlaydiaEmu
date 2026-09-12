# Architecture

## Strategy

LLE of the main Hitachi SuperH-1 (SH7032) CPU plus a modeled CDXA device.
Software talks to the co-processor through memory-mapped registers and shared
DRAM, not a stable HLE syscall ABI.

A disc-streaming path also demuxes CDS-XA sectors so video/audio can be
exercised without a user-supplied BIOS (retail boot still requires firmware).

## Workspace

```
crates/
├── playdia-core/       # headless deterministic core
├── playdia/            # standalone CLI / headless runner
├── playdia-libretro/   # libretro cdylib
└── playdia-tools/      # ISO/XA inspector
```

## Core contract

- `Machine::load_disc_*` / `load_bios_*`
- `reset`, `run_frame`, `set_input`
- `framebuffer` → 320×240 RGB555
- `drain_audio` → interleaved stereo i16 @ 44100
- `save_state` / `load_state` with content identity + CRC
- diagnostics for unmapped access and unknown opcodes

## Proven memory map (hardware access dump)

| Base | Size | Region |
|------|------|--------|
| 0x000F0000 | 0x100 | CDXA shared window |
| 0x00100000 | 384 KiB | CDXA stream DRAM |
| 0x00180000 | 96 KiB | CDXA VRAM |
| 0x00200000 | 512 KiB | CDXA work DRAM |
| 0x007FFC00 | 1 KiB | SH1 internal RAM |
| 0xE0000000 | 512 KiB | Main EPROM (BIOS) |

## CDS-XA routing

Mode 2 sectors:

- submode bit2 `0x04` → XA ADPCM audio (Form2 payload)
- channel 0 + submode bit3 `0x08` → data markers in payload[0]:
  - `0xF1` video fragment
  - `0xF2` frame end (submode bit0 clear) or interactive command (bit0 set)
  - `0xF3` scene reset

## Video

F1 fragments accumulate into a packet starting `00 80 04` (quant scale +
qtables). Decoder is a proprietary MPEG-1-like DCT path (192×144 4:2:0
centered in 320×240 RGB555). AC tables are still being locked against real
hardware references — see `docs/PROJECT-STATUS.md`.
