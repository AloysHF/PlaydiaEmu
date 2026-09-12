# Game compatibility

HLE player (no BIOS). Status from private Redump samples under `tmp/iso/`.

| Title | Load CUE | Stream track | Video frames | Audio PCM | Notes |
|-------|----------|--------------|--------------|-----------|-------|
| Mari-nee no Heya | 已验证 | 已验证 (Track2) | 已验证 (96 host / 96 decoded) | 已验证 (~846k samples) | DC-only image |
| Playdia Sample Soft | 已验证 | 已验证 | 已验证 | 已验证 | DC-only image |
| Other Redump titles | 未验证 | 结构同 | 未验证 | 未验证 | Expected same CUE layout |

Legend: 已验证 / 部分 / 未验证

## How to update

```powershell
cargo run --release -p playdia -- inspect "tmp/iso/<title>/<title>.cue"
cargo run --release -p playdia -- play "tmp/iso/<title>/<title>.cue" --frames 120 --dump-ppm tmp/out/<title>.ppm
```

Do not commit discs, BIOS, or PPM dumps.
