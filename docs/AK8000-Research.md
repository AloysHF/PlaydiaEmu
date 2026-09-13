# AK8000 framing research

## Evidence and sources

[Furrtek's AK8000 die post](https://x.com/furrtek/status/1789990207179112492)
identifies a Hitachi HG51 standard-cell custom chip with RAM. It does not
identify an MPEG decoder or provide a ROM dump. The public
[SiliconRE repository](https://github.com/furrtek/SiliconRE) includes a die
overview, but the material inspected does not provide an AK8000 netlist or
entropy lookup table.

[Asahi's JPH06178281A patent](https://patents.google.com/patent/JPH06178281A/en),
filed in 1992 and published in 1994, describes 4×4 transform blocks, six blocks
per macroblock, 31 macroblocks per row and 26 rows (248×208 pixels). Its picture
header includes PSC, PTYPE, QBS, factors and Y/C quantizers; row headers include
LMBSC and a number from 1 to 26. It requires 186 block-end symbols per row and
describes a Huffman ROM, nonlinear dequantization, prediction and inverse
Hadamard transformation. The actual Huffman codebook is not given. This is a
strong architectural lead, not proof of every AK8000 operation.

[pyplaydia revision 1611f64](https://github.com/larrykoubiak/pyplaydia/tree/1611f64)
provided the F2-tail and bit-field leads tested here. Its older research notes
contain conflicting codec models. In particular, a mandatory fixed DC token
after every row marker is not assumed by this implementation.

## Independently checked disc structure

Full scans of two private data tracks produced the following results. No disc
content or hardware capture is included in this repository.

| Check | Track A | Track B |
|---|---:|---:|
| Assembled video packets | 10,910 | 11,053 |
| Ordered row 1..26 candidates plus padding-anchored terminator | 10,910 | 11,053 |
| Packets with multiple possible ordered marker sequences | 4,749 | 6,340 |
| Terminators requiring F2 video bytes | 10,839 | 10,730 |
| FF-filled F3 sectors | 336 | 387 |
| F3 padding encountered with pending video | 0 | 51 |

Each F1 contributes user bytes [1..2048]; noninteractive F2 contributes
[0x23..2048]. Dropping F2 removes actual compressed data from most packets.
F3 sectors containing a two-byte word followed by FF padding must preserve
the accumulator. Unknown F3 layouts retain legacy handling.

The 36-byte picture header is followed by MSB-first 14-bit 0x20 / 5-bit row
markers. Bytes 38/39 are not independent segment/flag fields. A final 14-bit
0x21 precedes zero bits and FF padding. That marker can occur inside entropy,
so taking the first occurrence truncates real pictures. The scanner anchors
it to padding and reports ambiguity in the ordered row sequence instead of
claiming validated entropy boundaries.

## Reproduction and next decoding step

```powershell
cargo run --release -p playdia-tools --bin playdia-inspect -- path\to\game.cue --video-rows
```

The core and inspector share fragment slicing and padding recognition. Tests
cover F2 continuation, interleaved F3 padding, accumulator overflow, unaligned
row markers, false terminators and ambiguous row sequences.

The current rendered preview still uses the earlier speculative 192×144,
8×8 codec. It does not produce original game pixels. The next entropy decoder
must consume whole rows, validate exactly 186 block ends under the patent
hypothesis, reject unresolved codewords, and check coefficient/prediction/
transform rules before replacing the preview. Marker matching alone cannot
establish a Huffman table, a correct image or hardware pixel accuracy.

A separate exploratory probe combined the published short ladder codes,
absolute-DC token and a proposed long escape family on 260 candidate rows
from a third disc. None parsed to the row end under that partial grammar.
This rejects that incomplete implementation, not every possible completion of
the codebook. No guessed coefficient decoder was added to the playback path.
