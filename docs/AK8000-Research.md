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
| Assembled video packets, including interactive F2 | 10,957 | 11,283 |
| Ordered row 1..26 candidates plus padding-anchored terminator | 10,957 | 11,283 |
| Packets with multiple possible ordered marker sequences | 4,750 | 6,401 |
| Terminators requiring F2 video bytes | 10,884 | 10,925 |
| Pictures ending at interactive F2 | 47 | 230 |
| Interactive pictures requiring F2 tail bytes | 45 | 195 |
| FF-filled F3 sectors | 336 | 387 |
| F3 padding encountered with pending video | 0 | 51 |

Each F1 contributes user bytes [1..2048]; every F2 contributes
[0x23..2048]. Dropping F2 removes actual compressed data from most packets.
F3 sectors containing a two-byte word followed by FF padding must preserve
the accumulator. Unknown F3 layouts retain legacy handling.

Across four complete private tracks, all 81,480 assembled packets contain one
initial picture header, an ordered 26-row sequence and an anchored terminator.
Of these, 3,210 end at interactive F2; 914 require its tail bytes. Treating these
F2 sectors as commands alone merges multiple pictures, sometimes overflowing
the accumulator. The HLE player now finishes video and presents the latest
queued preview before applying the command. The older two-track count of
21,963 excluded 277 interactive pictures; it did not represent all pictures.

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

## Sparse codeword experiment

```powershell
python tools/probe_sparse_vlc.py path\to\track.bin
python tools/probe_sparse_vlc.py path\to\disc.zip --candidate-family --gamma
```

The standard-library-only probe considers packets shorter than 7,500 trimmed
bytes and candidate rows shorter than 650 bits. Its default grammar recognizes
`01`, `0010000000` plus 10 bits, and short ladder-shaped tokens. It does not
assign coefficient values, zero-run lengths, components or pixels.

| Conservative grammar | Discovery track | Separate validation track |
|---|---:|---:|
| Candidate short rows | 1,302 | 1,829 |
| Complete consumption with exactly 186 EOB candidates | 432 | 587 |
| Distinct bitstrings among those rows | 406 | 494 |
| Complete consumption with a wrong EOB count | 0 | 6 |
| Remaining unresolved rows | 870 | 1,236 |

The validation track contains 37 rows consisting of the 20-bit prefix followed
by exactly 186 repetitions of `01`. This supports the block-count hypothesis.
The inferred `00001000` token and long gamma-like escapes sometimes increase
successful parses but also increase wrong counts; both stay opt-in. Even a
186-count match does not prove that every consumed token boundary is correct.
