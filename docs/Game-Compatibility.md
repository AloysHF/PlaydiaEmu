# Game Compatibility

HLE player (no BIOS). Status is based on private Redump samples; do not commit
discs, BIOS images, or PPM dumps.

## Summary

| Status | Count |
|--------|-------|
| HLE player integration checks (load + stream + video + audio) | 2 |
| Native entropy validation across complete disc streams | 37 |
| Hardware pixel comparison or complete playthrough verified | 0 |

## Game List

| Title | Load CUE | Stream track | Video frames | Audio PCM | Notes |
|-------|----------|--------------|--------------|-----------|-------|
| Mari-nee no Heya | ✅ | ✅ (Track 2) | ✅ (180 host / 103 decoded) | ✅ | Native title and Push B prompt; waits for input |
| Playdia Sample Soft | ✅ | ✅ | ✅ | ✅ | Native menu and demo pictures |
| Other tested titles | ⬜ | Validated stream packets | See corpus report | ⬜ | Entropy checks do not establish navigation or full compatibility |

Legend: ✅ verified · ⬜ not yet verified

The [full corpus report](AK8000-Corpus-Validation.md) covers 1,135,539 packets
from 37 discs, including two single-track titles. Eight Aqua Adventure packets
are truncated in the last row; all remaining packets pass strict decoding.

## How to update

Inspect a title:

```powershell
cargo run --release -p playdia-tools --bin playdia-inspect -- "tmp/iso/<title>/<title>.cue"
```

Play headlessly and dump a frame:

```powershell
cargo run --release -p playdiaemu -- `
  "tmp/iso/<title>/<title>.cue" `
  --headless --frames 120
```

Record:

1. Whether the CUE loads
2. Stream track number and F1/F2/F3 / audio sector counts from `playdia-inspect`
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
