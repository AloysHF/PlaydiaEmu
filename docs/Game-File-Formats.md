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
- `0xF2` — append payload[0x23..0x800] and end pending video; submode bit 0
  additionally selects interactive command handling
- `0xF3` — preserve pending video for 2048-byte sectors whose bytes[3..] are FF;
  other F3 forms retain the legacy reset behavior pending further evidence

Interactive F2 sectors (`submode` bit 0 set) contain a command byte followed by
seven four-byte button destinations. Each destination stores binary minute,
second, frame, and an extra byte; the absolute disc LBA is
`minute × 4500 + second × 75 + frame − 150`. The HLE player follows known
scene jumps and button choices within the stream track. Other command effects
remain under investigation.
Interactive F2 sectors also carry video tails. The HLE player finishes and
presents the latest queued preview before handling the control command, so
waiting for input does not freeze on an earlier queued picture.

A complete frame packet typically starts with `00 80 04` (quant scale +
qtables), then F1 fragments, then F2 end.

The picture header is 36 bytes: a 19-bit picture start code (0x400), 3-bit
picture type, 2-bit quantizer shift, 8-bit factor and two 16-byte tables.
MSB-first row markers follow: 14-bit 0x20 and a 5-bit row number (1..27).
Consequently bytes 38 and 39 span the row number and entropy data; the previous
independent "segment code" and "flags" interpretation was incorrect.
The inspector now labels byte 38 as raw data. It anchors the 14-bit 0x21
terminator to final zero bits and FF padding, since that pattern also occurs
inside entropy. Ordered row marker matches can still be ambiguous.

## Video path (recovered syntax)

The default decoder reads one 248×216 picture per assembled packet and centers
it in the 320×240 XRGB8888 framebuffer. Each of 27 rows contains 31 macroblocks
of four luma and two chroma 4×4 blocks. Entropy uses signed run/level VLCs,
`01` EOB and a six-bit `001000` escape with four run bits and ten signed level
bits. A block filled through coefficient 15 ends without another EOB.

DC differences for Y1 refer to the preceding macroblock's Y1. Y2/Y3/Y4 refer
to the current Y1; Cb and Cr have independent predictors. Predictors reset
at each row. Reconstruction currently uses factor × quantizer / 64 and a
fixed-point 4×4 inverse DCT, followed by YCbCr conversion. Separate luma and
chroma tables are retained. Rare VLCs, nonlinear quantization, hardware
transform rounding and analog color conversion remain research questions.
See [AK8000 research](AK8000-Research.md) for evidence and limitations.
`playdia-frame`, playback and PPM screenshots retain the same eight-bit RGB
channels. Decoded packets contain packed R/G/B bytes; framebuffers use `u32`
words in `0x00RRGGBB` order. This software output format does not establish
the original hardware color precision.

Raw framebuffer dumps and framebuffer CRCs use four little-endian bytes per
pixel (B, G, R, 0), totaling 307,200 bytes for 320×240. Save-state version 2
stores this format; version 1 RGB555 states are rejected before loading.

The earlier 26-row assumption merged rows 26 and 27. Counting 186 literal
EOBs was also insufficient because full blocks omit EOB. The native path
validates coefficient bounds and the next marker at the exact consumed bit
position; it does not search ahead to hide entropy errors.
An independent survey of 35 data-track discs found the expected initial packet
prefix in all 900,268 assembled video packets; it did not validate picture decode.
The old 192×144 preview is retained for the parameter-sweep research tool via
`CodecParams::legacy_preview`. Its bit-order and AC options do not affect
native playback.

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
  - `0xF3` FF-filled padding (other forms retain legacy reset handling)

PID/subheader conventions seen on real discs: `0x61` video, `0x62` audio.
