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
| Mari-nee no Heya | ✅ | ✅ (Track 2) | ✅ (96 host / 96 decoded) | ✅ (~846k samples) | Approximate image |
| Playdia Sample Soft | ✅ | ✅ | ✅ | ✅ | Approximate image |
| Other Redump titles | ⬜ | Expected same | ⬜ | ⬜ | Expected same CUE layout |

Legend: ✅ verified · ⬜ not yet verified

## How to update

Inspect a title:

```powershell
cargo run --release -p playdiaemu -- inspect "tmp/iso/<title>/<title>.cue"
```

Play headlessly and dump a frame:

```powershell
cargo run --release -p playdiaemu -- play `
  "tmp/iso/<title>/<title>.cue" `
  --frames 120 --dump-ppm tmp/out/<title>.ppm
```

Record:

1. Whether the CUE loads
2. Stream track number and F1/F2/F3 / audio sector counts from `inspect`
3. Host frames run and whether the PPM is non-blank
4. Approximate PCM sample count from logs

## Notes

- Default video uses the recovered **248×216 AK8000 decoder**. Sample Soft,
  Dragon Ball Z and Mari-nee pictures are recognizable; this does not establish
  hardware pixel accuracy or full game compatibility. Use `playdia-frame
  --check-all` to measure entropy coverage separately from navigation.
- F2 jump and button-choice navigation is exercised on a private disc; timeout,
  quiz, and score semantics have not been verified on hardware.
- Prefer `--release` builds for any visual check.
- Do not commit discs, BIOS, or PPM dumps.
