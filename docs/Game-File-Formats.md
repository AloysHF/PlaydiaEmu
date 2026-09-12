# Game File Formats

Playdia software ships as CDS-XA disc images. This document describes the disc
layout, sector structure, audio groups, and video markers used by the HLE
player.

## Disc images

| Kind | Notes |
|------|-------|
| Dual-track CUE/BIN | Preferred. Raw 2352-byte Mode2 sectors. |
| Cooked ISO9660 | 2048-byte sectors; no XA realtime path. |

Typical layout:

- **Track 1** — ISO9660 data (file IDs such as `0A000001.DAT`)
- **Track 2** — interactive FMV / audio stream (the payload the HLE player consumes)

## Mode 2 subheader (bytes 16..24)

| Offset | Field |
|--------|-------|
| 0 | file id |
| 1 | channel id |
| 2 | submode |
| 3 | coding |

### Submode bits used by Playdia

| Bit | Mask | Meaning |
|-----|------|---------|
| 0 | 0x01 | interactive (with F2) |
| 2 | 0x04 | audio |
| 3 | 0x08 | data |
| 5 | 0x20 | Form 2 |

## Form 2 audio payload

18 × 128-byte sound groups. Each group: 8 sound units, 28 samples/unit,
4-bit ADPCM, filters `{0,60,115,98}/{0,0,-52,-55}` ×64.

Native rate 37800 Hz or 18900 Hz (coding bit2); resampled to 44100 stereo.

## Video markers (Form1 data, channel 0, submode 0x08)

Payload[0]:

- `0xF1` — append payload[1..] to frame accumulator
- `0xF2` — end of frame (decode if acc starts `00 80 04`) or interactive cmd
- `0xF3` — reset accumulator

Interactive F2 sectors (`submode` bit 0 set) contain a command byte followed by
seven four-byte button destinations. Each destination stores binary minute,
second, frame, and an extra byte; the absolute disc LBA is
`minute × 4500 + second × 75 + frame − 150`. The HLE player follows known
scene jumps and button choices within the stream track. Other command effects
remain under investigation.

A complete frame packet typically starts with `00 80 04` (quant scale +
qtables), then F1 fragments, then F2 end.

The first 40 bytes contain a quant scale, two 16-byte quantizer tables, and a
secondary `00 80 XX` marker with a following flag byte. The secondary code is
not always `0x24`; other values appear on real discs. Its meaning and the
number of display pictures represented by each packet remain unverified.

## Video path (approximate)

The decoder is a proprietary MPEG-1-like DCT path targeting 320×240 RGB555.
The current default reads a fixed number of raw AC coefficients per block;
`--full-decode` applies experimental quantization scaling. Pixel-accurate
AK8000 VLC is still unsolved — treat
frames as approximate unless fixture-proven against real hardware references.

## Memory map (LLE / hardware access dump)

| Base | Size | Region |
|------|------|--------|
| 0x000F0000 | 0x100 | CDXA shared window |
| 0x00100000 | 384 KiB | CDXA stream DRAM |
| 0x00180000 | 96 KiB | CDXA VRAM |
| 0x00200000 | 512 KiB | CDXA work DRAM |
| 0x007FFC00 | 1 KiB | SH1 internal RAM |
| 0xE0000000 | 512 KiB | Main EPROM (BIOS) |

Retail LLE boot needs a user-supplied 512 KiB BIOS at `0xE0000000`.

## CDS-XA routing summary

Mode 2 sectors:

- submode bit2 `0x04` → XA ADPCM audio (Form2 payload)
- channel 0 + submode bit3 `0x08` → data markers in payload[0]:
  - `0xF1` video fragment
  - `0xF2` frame end (submode bit0 clear) or interactive command (bit0 set)
  - `0xF3` scene reset

PID/subheader conventions seen on real discs: `0x61` video, `0x62` audio.
