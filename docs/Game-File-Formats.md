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
- `0xF2` — append payload[0x23..0x800] and end the packet, or interactive command
- `0xF3` — preserve pending video for 2048-byte sectors whose bytes[3..] are FF;
  other F3 forms retain the legacy reset behavior pending further evidence

Interactive F2 sectors (`submode` bit 0 set) contain a command byte followed by
seven four-byte button destinations. Each destination stores binary minute,
second, frame, and an extra byte; the absolute disc LBA is
`minute × 4500 + second × 75 + frame − 150`. The HLE player follows known
scene jumps and button choices within the stream track. Other command effects
remain under investigation.

A complete frame packet typically starts with `00 80 04` (quant scale +
qtables), then F1 fragments, then F2 end.

The picture header is 36 bytes: a 19-bit picture start code (0x400), 3-bit
picture type, 2-bit quantizer shift, 8-bit factor and two 16-byte tables.
MSB-first row markers follow: 14-bit 0x20 and a 5-bit row number (1..26).
Consequently bytes 38 and 39 span the row number and entropy data; the previous
independent "segment code" and "flags" interpretation was incorrect.
The inspector now labels byte 38 as raw data. It anchors the 14-bit 0x21
terminator to final zero bits and FF padding, since that pattern also occurs
inside entropy. Ordered row marker matches can still be ambiguous.

## Video path (approximate)

The display is 320×240 RGB555. The legacy preview uses a speculative 8×8 DCT
path and does not implement the observed row format. An Asahi patent provides
a much stronger 4×4 transform / 248×208 picture hypothesis, but its Huffman
table and pixel reconstruction are not yet recovered; see
[AK8000 research](AK8000-Research.md).
The current default reads a fixed number of raw AC coefficients per block;
`--full-decode` applies experimental quantization scaling. Pixel-accurate
AK8000 VLC is still unsolved — treat
frames as approximate unless fixture-proven against real hardware references.
Some short packets contain long `0x55`/`0xAA` byte runs. The inspect tool can
locate them with `--video-candidates`, but their codeword and pixel meanings
remain unverified.
An independent survey of 35 data-track discs found the expected initial packet
prefix in all 900,268 assembled video packets; it did not validate picture decode.
The experimental decoder's `lsb_first` parameter now selects the actual entropy
byte bit order. Its default preserves the previous LSB-first preview behavior;
the separate picture structure scanner uses the observed MSB-first framing.

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
