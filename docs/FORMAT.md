# Content format

## Disc images

- Raw 2352-byte Mode1/Mode2 sectors (preferred)
- Cooked 2048-byte ISO9660 (no XA realtime path)

## Mode 2 subheader (bytes 16..24)

| Offset | Field |
|--------|-------|
| 0 | file id |
| 1 | channel id |
| 2 | submode |
| 3 | coding |

Submode bits used by Playdia:

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
