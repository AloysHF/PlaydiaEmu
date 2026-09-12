# Game compatibility

| Title / content | Disc load | SH-1 boot | CDXA video | CDXA audio | Notes |
|-----------------|-----------|-----------|------------|------------|-------|
| Synthetic F1/F2 fixtures | 已验证 | n/a (placeholder BIOS) | 部分（累积+解码入口） | 部分（XA ADPCM 结构） | unit tests |
| Real title + user BIOS | 未验证 | 未验证 | 未验证 | 未验证 | needs BIOS dump + ISO |
| Real title, no BIOS (stream only) | 部分 | 无 | 依赖解码正确性 | 依赖 ADPCM 锁定 | streaming path only |

Legend: 已验证 / 部分 / 未验证

## How to update this matrix

1. Place a legal BIOS dump at `tmp/bios/playdia_bios.bin` (private).
2. Place a disc image at `tmp/iso/*.iso` (private).
3. Run:

```powershell
cargo run -p playdia -- headless path\to\disc.iso --bios path\to\bios.bin --frames 180
```

4. Record frame CRC, CPU PC, and diagnostics. Do not commit ROMs/BIOS/ISOs.
