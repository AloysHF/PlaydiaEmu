# AK8000 decoding research

Current decoder model, evidence, and reproduction only. Superseded framing
studies, sparse-probe tables, and intermediate recovery counts are local
research notes (not in this repository).

## Current decoder

The default Rust decoder reconstructs recognizable disc pictures (for example
Sample Soft INTERACTIVE / NO WAIT & HIGH SPEED, Dragon Ball Z scenes, and
Mari-nee’s title and button prompt) from compressed disc coefficients. No
reference image, generated artwork, or frame substitution is used. Hardware
pixel accuracy has not been established.

Facts the native path relies on:

- **27 rows** per picture; dimensions **248×216**. Sequential coefficient
  decoding reaches a row-27 marker then the terminal `0x21`.
- Blocks filled through coefficient 15 **omit EOB**.
- DC prediction: **Y1** uses the preceding macroblock’s Y1; **Y2/Y3/Y4** use
  the current Y1; Cb/Cr are independent; all predictors reset each row.

Entropy: `01` EOB, signed run/level symbols, and `001000` escape + four run
bits + signed ten-bit level. Escape levels are differences, including at
coefficient zero. The short-code tree resembles parts of H.261 after branch
rearrangement, but H.261’s run/level table fails 16-coefficient bounds and is
**not** used as the native mapping.

Reconstruction: separate Y/C tables, factor × quantizer / 64, 4×4 zigzag,
fixed-point inverse DCT, then YCbCr. The patent’s figure 4 supports the
factor/64 lead. Nonlinear dequantization, exact hardware transform, and color
rounding still need comparison with trustworthy captures or chip logic.

Reproduce with `playdia-frame` (`--packet` / `--check-all`); flags and full
commands: [Tools](Tools.md#playdia-frame). The frame tool and player export
248×216 RGB888/XRGB8888 pixels. Frontends present the native frame at the
provisional 4:3 display aspect. A picture is only presented after all rows and
padding validate. RGB888 is the software
reconstruction format, not a verified hardware precision claim. Decode
failures retain the last complete picture.

Synthetic tests cover macroblock prediction, signed escapes, implicit block
ends, truncated packets, coefficient overflow, video in interactive F2 tails,
cached presentation, PPM channel order, state version rejection, and libretro
format negotiation. Proprietary pictures are not test fixtures.

Full-disc native validation (regression tracks used during recovery):

| Disc | Pictures | All rows, blocks and trailer valid |
|---|---:|---:|
| Mari-nee no Heya | 10,957 | 10,957 |
| Yumi to Tokoton Playdia | 11,283 | 11,283 |

A 180-host-frame Mari-nee run presents 103 pictures with zero failures and
reaches the title/button-choice screen. Entropy coverage does not prove
decoded pixel values.

All 37 supplied archives: **1,135,531 of 1,135,539** pictures pass. The eight
Aqua Adventure failures exhaust data in row 27 without a terminal marker;
checked F2 EDCs match. See the [full corpus report](AK8000-Corpus-Validation.md).

## Evidence and sources

[Furrtek’s AK8000 die post](https://x.com/furrtek/status/1789990207179112492)
identifies a Hitachi HG51 standard-cell custom chip with RAM. It does not
identify an MPEG decoder or provide a ROM dump. The public
[SiliconRE repository](https://github.com/furrtek/SiliconRE) includes a die
overview, but the material inspected does not provide an AK8000 netlist or
entropy lookup table.

[Asahi’s JPH06178281A patent](https://patents.google.com/patent/JPH06178281A/en)
(filed 1992, published 1994) describes 4×4 transform blocks, six blocks per
macroblock, 31 macroblocks per row and 26 rows (248×208 pixels). Picture
header: PSC, PTYPE, QBS, factors, Y/C quantizers; row headers: LMBSC and
number 1..26. It requires 186 block-end symbols per row and describes a
Huffman ROM, nonlinear dequantization, prediction, and inverse Hadamard. The
actual Huffman codebook is not given. Strong architectural lead, not proof of
every AK8000 operation.

[pyplaydia revision 1611f64](https://github.com/larrykoubiak/pyplaydia/tree/1611f64)
provided F2-tail and bit-field leads. Its notes include conflicting codec
models; this implementation does **not** assume a mandatory fixed DC token
after every row marker.

## Inspection helpers

Row-marker and packet scans: `playdia-inspect --video-rows` (see
[Tools](Tools.md#playdia-inspect)). Core and inspector share fragment slicing
and padding recognition. Tests cover F2 continuation, interleaved F3 padding,
accumulator overflow, unaligned row markers, false terminators, and ambiguous
row sequences.
