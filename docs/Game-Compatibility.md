# Game Compatibility

HLE player (no BIOS). Status is based on private Redump samples; do not commit
discs, BIOS images, or PPM dumps.

## Summary

| Status | Count |
|--------|-------|
| ✅ Verified (load + stream + video + audio) | 2 |
| ⬜ Unlisted Redump titles | Many (same dual-track CUE layout expected) |

## Game List

| Title | Load CUE | Stream track | Video frames | Audio PCM | Notes |
|-------|----------|--------------|--------------|-----------|-------|
| Mari-nee no Heya | ✅ | ✅ (Track 2) | ✅ (96 host / 96 decoded) | ✅ (~846k samples) | DC-only image |
| Playdia Sample Soft | ✅ | ✅ | ✅ | ✅ | DC-only image |
| Other Redump titles | ⬜ | Expected same | ⬜ | ⬜ | Expected same CUE layout |

Legend: ✅ verified · ⬜ not yet verified

## How to update

Inspect a title:

```powershell
cargo run --release -p playdia -- inspect "tmp/iso/<title>/<title>.cue"
```

Play headlessly and dump a frame:

```powershell
cargo run --release -p playdia -- play `
  "tmp/iso/<title>/<title>.cue" `
  --frames 120 --dump-ppm tmp/out/<title>.ppm
```

Record:

1. Whether the CUE loads
2. Stream track number and F1/F2/F3 / audio sector counts from `inspect`
3. Host frames run and whether the PPM is non-blank
4. Approximate PCM sample count from logs

## Notes

- Default video path is **DC-only reconstruction**; frames are structured but
  not pixel-accurate until the AK8000 VLC is locked.
- Prefer `--release` builds for any visual check.
- Do not commit discs, BIOS, or PPM dumps.
