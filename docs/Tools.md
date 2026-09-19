# Tools (`playdiaemu-tools`)

Research and validation CLIs in the `playdiaemu-tools` package. Prefer
`--release` for any real-disc run.

```text
playdia-inspect         Sector / packet diagnostics
playdia-frame           Decode one picture or validate every picture
playdia-scan            Sweep CodecParams against a reference frame
batch-screenshots.ps1   Drive standalone -S over a folder of discs
```

## playdia-inspect

Print disc kind, sector counts, CRC, ISO volume-label sample, and stream-track
F1/F2/F3 / audio sector counts:

```powershell
cargo run --release -p playdiaemu-tools --bin playdia-inspect -- path\to\game.cue
cargo run --release -p playdiaemu-tools --bin playdia-inspect -- path\to\game.cue --video-headers
cargo run --release -p playdiaemu-tools --bin playdia-inspect -- path\to\game.cue --video-rows
cargo run --release -p playdiaemu-tools --bin playdia-inspect -- path\to\game.cue --video-candidates
```

| Flag | Report |
|------|--------|
| *(none)* | Disc kind, tracks, CRC, sample volume label, F1/F2/F3 / audio counts |
| `--video-headers` | Assembled video packet headers and length frequencies (no export) |
| `--video-rows` | Ordered 27-row candidates, ambiguous matches, padding-anchored terminators |
| `--video-candidates` | Low-entropy bodies and long `0x55`/`0xAA` runs, with track-relative LBAs |

Candidate and header reports are encoded bitstream patterns, not evidence of
pixel-accurate decoding. F2 overflow contributes video bytes; FF-filled F3
sectors preserve pending video. Interactive F2 finishes pending video before a
choice or jump. Decoder facts: [AK8000 research](AK8000-Research.md).

## playdia-frame

Decode a single native picture or validate every picture on the disc. Reads
pictures in disc order without following scene commands.

```powershell
cargo run --release -p playdiaemu-tools --bin playdia-frame -- game.cue --packet 523 --output scene.ppm
cargo run --release -p playdiaemu-tools --bin playdia-frame -- game.cue --check-all
```

| Option | Meaning |
|--------|---------|
| `--packet N` | One-based picture index (includes interactive F2 packets) |
| `--output PATH` | Write the selected picture as 248×216 RGB888 PPM |
| `--check-all` | Validate every picture; exit non-zero if any fail (conflicts with `--output`) |
| `--assembled` | Treat input as one assembled packet instead of a disc image |

`--check-all` reports failed indices, track-relative LBAs, rows, blocks, and
bit offsets. CUE and raw MODE2/2352 BIN are supported. Player PPM screenshots
use the same 248×216 native geometry and channel precision.

Corpus totals: see [AK8000 corpus validation](AK8000-Corpus-Validation.md)
(entropy coverage only — not hardware pixels or full-game compatibility).

## playdia-scan

Sweep video codec parameters against a reference frame (parameter research;
does not replace native playback defaults):

```powershell
cargo run --release -p playdiaemu-tools --bin playdia-scan -- game.cue reference.ppm --max-frames 300 --top 12
```

| Option | Default | Meaning |
|--------|---------|---------|
| `DISC` | required | Disc path |
| `REFERENCE` | required | Reference gray/PPM (or raw 192×144×3 bytes) |
| `--max-frames` | `300` | Cap on frames cached/scored |
| `--top` | `12` | How many parameter sets to print |
| `--dump-best` | — | Directory for best-scoring dumps |

## Batch screenshots (`scripts/batch-screenshots.ps1`)

Shell script (not in the Rust tools crate). Drives the standalone binary in
screenshot mode. Captures a PNG for every disc under a folder. Output goes to
`docs/images/`. Redump-style ZIPs are loaded in place:

```powershell
# Rebuild release binary, then capture every .zip under -RedumpDir
.\scripts\batch-screenshots.ps1 -RedumpDir <local-redump-folder>

# Existing binary, extracted CUE/BIN tree under tmp\playdia_game
.\scripts\batch-screenshots.ps1 -SkipBuild -GameDir tmp\playdia_game
```

Optional: `-Frames` (default 300, with a few title-screen overrides that may
also inject `--press-at` to leave the BANDAI boot logo), `-TimeoutSeconds`
(default 300), `-Binary`, `-SkipBuild`. Do not commit disc images; only the
PNG previews and the script belong in the repository. See
[Game Compatibility](Game-Compatibility.md) for the published matrix.

## See also

- [Standalone Emulator](Standalone-Emulator.md)
- [AK8000 research](AK8000-Research.md)
- [Game Compatibility](Game-Compatibility.md)
- [Game File Formats](Game-File-Formats.md)
