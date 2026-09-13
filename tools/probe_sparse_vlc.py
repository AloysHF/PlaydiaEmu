"""Test partial AK8000 codeword shapes; this does not decode coefficients or pixels."""

import argparse
from collections import Counter
from contextlib import contextmanager
import json
from pathlib import Path
import re
import zipfile

SYNC = f"{0x20:014b}"
TERMINATOR = f"{0x21:014b}"
BITS = tuple(f"{b:08b}" for b in range(256))
LADDER = re.compile(r"(?:00|000)?10{0,5}1[01]")
GAMMA = re.compile(r"0{5,14}1")


@contextmanager
def open_track(path):
    if path.suffix.lower() == ".zip":
        with zipfile.ZipFile(path) as archive:
            tracks = [n for n in archive.namelist() if n.lower().endswith("(track 2).bin")]
            if len(tracks) != 1:
                raise ValueError("Expected exactly one Track 2 BIN in the ZIP")
            with archive.open(tracks[0]) as stream:
                yield stream
    else:
        with path.open("rb") as stream:
            yield stream


def packets(stream):
    packet = bytearray()
    overflow = False
    while raw := stream.read(2352):
        if len(raw) != 2352:
            raise ValueError("Truncated MODE2/2352 sector")
        if raw[15] != 2 or raw[16] != 1 or raw[17] != 0 or not raw[18] & 8:
            continue
        data = raw[24:2072]
        if data[0] == 0xF1:
            if len(packet) + 2047 <= 256 * 1024 and not overflow:
                packet.extend(data[1:])
            else:
                overflow = True
        elif data[0] == 0xF2:
            if packet and not overflow and len(packet) + 2013 <= 256 * 1024:
                packet.extend(data[0x23:])
                yield bytes(packet)
            packet.clear()
            overflow = False
        elif data[0] == 0xF3 and any(b != 255 for b in data[3:]):
            packet.clear()
            overflow = False


def candidate_rows(packet):
    bits = "".join(BITS[b] for b in packet.rstrip(b"\xff"))
    if bits[:19] != f"{0x400:019b}":
        return
    end = bits.rfind("1") + 1 - 14
    if end < 288 or bits[end:end + 14] != TERMINATOR or len(bits) - end - 14 > 15:
        return
    if bits[288:307] != SYNC + "00001":
        return
    starts = [288]
    for row in range(2, 27):
        pos = bits.find(SYNC + f"{row:05b}", starts[-1] + 19, end)
        if pos < 0:
            return
        starts.append(pos)
    for start, stop in zip(starts, starts[1:] + [end]):
        yield bits[start + 19:stop]


def probe(bits, candidate_family=False, gamma=False):
    """Return consumed bits and EOB count under an incomplete grammar."""
    pos = blocks = 0
    while pos < len(bits):
        if bits.startswith("01", pos):
            pos += 2
            blocks += 1
        elif bits.startswith("0010000000", pos):
            if pos + 20 > len(bits):
                break
            pos += 20
        elif candidate_family and bits.startswith("00001000", pos):
            pos += 8
        elif match := LADDER.match(bits, pos):
            pos = match.end()
        elif gamma and (match := GAMMA.match(bits, pos)):
            length = 2 * len(match[0])
            if pos + length > len(bits):
                break
            pos += length
        else:
            break
    return pos, blocks


def main():
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("track", type=Path, help="Raw MODE2/2352 BIN or a ZIP containing Track 2")
    cli.add_argument("--packet-bytes", type=int, default=7500, help="Maximum trimmed packet bytes (exclusive)")
    cli.add_argument("--row-bits", type=int, default=650, help="Maximum candidate row bits (exclusive)")
    cli.add_argument("--candidate-family", action="store_true", help="Try the inferred 00001000 token")
    cli.add_argument("--gamma", action="store_true", help="Try unverified long escapes; can increase wrong block counts")
    args = cli.parse_args()
    counts = Counter()
    unique = set()
    with open_track(args.track) as stream:
        for packet in packets(stream):
            counts["packets"] += 1
            if len(packet.rstrip(b"\xff")) >= args.packet_bytes:
                continue
            counts["short_packets"] += 1
            for row in candidate_rows(packet):
                if len(row) >= args.row_bits:
                    continue
                counts["candidate_rows"] += 1
                if row.startswith("0010000000") and row[20:] == "01" * 186:
                    counts["dc_prefix_plus_186_eob"] += 1
                consumed, blocks = probe(row, args.candidate_family, args.gamma)
                if consumed == len(row) and blocks == 186:
                    counts["complete_186"] += 1
                    unique.add(row)
                elif consumed == len(row):
                    counts["complete_wrong_count"] += 1
                else:
                    counts["unresolved"] += 1
    counts["unique_complete_186"] = len(unique)
    print(json.dumps(dict(sorted(counts.items())), indent=2))
    print("Candidate marker boundaries and codeword shapes only; no coefficient or pixel validation.")


if __name__ == "__main__":
    main()
