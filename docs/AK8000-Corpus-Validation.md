# Native AK8000 corpus validation

The native Rust entropy decoder was applied to all picture packets in 37 local
disc archives: 35 dual-track titles and two single-track MODE2/2352 titles.
Compressed bytes were assembled from F1 and F2 sectors, including interactive
F2 tails and intervening F3 padding. Validation date: 2026-09-13.

**1,135,531 of 1,135,539 packets pass** (99.9993%). All pictures in 36 discs
pass. Success requires exactly 27 ordered rows, 186 blocks per row, bounded
16-coefficient blocks and the terminal code plus valid padding. Filled blocks
end implicitly; the validator does not require 186 literal EOBs or resynchronize
by scanning for a later row marker.

These are structural decoder results, not hardware pixel comparisons or
complete game compatibility scores. The aggregate is a regression/coverage
result, not a held-out accuracy claim. The entropy table was unchanged during
the full 37-disc pass.

## Reproduction

Extract a local disc archive and run `playdia-frame --check-all` (optional
`--packet` PPM export). Commands and flags: [Tools](Tools.md#playdia-frame).
The CLI accepts ZIP archives, CUE images and raw MODE2/2352 BIN tracks.
Packet indices are one-based and include interactive F2 pictures. The command
does not follow scene navigation. No disc bytes or screenshots are distributed
with these results.

## Results

| Disc | Packets | Valid | Rejected |
|---|---:|---:|---:|
| Aqua Adventure - Blue Lilty (Japan) | 39,109 | 39,101 | 8 |
| Bandai Item Collection 70' (Japan) | 21,074 | 21,074 | 0 |
| Bishoujo Senshi Sailor Moon S - Quiz Taiketsu! Sailor Power Shuketsu!! (Japan) | 23,501 | 23,501 | 0 |
| Chougoukin Selections (Japan) | 34,614 | 34,614 | 0 |
| Dragon Ball Z - Shin Saiyajin Zetsumetsu Keikaku - Chikyuu-hen (Japan) | 31,774 | 31,774 | 0 |
| Dragon Ball Z - Shin Saiyajin Zetsumetsu Keikaku - Uchuu-hen (Japan) | 32,565 | 32,565 | 0 |
| Elements Voice Series - Aya Hisakawa - Forest Sways (Japan) | 40,741 | 40,741 | 0 |
| Elements Voice Series - Mariko Kouda - Welcome to the Marikotown! (Japan) | 43,077 | 43,077 | 0 |
| Elements Voice Series - Mika Kanai - Wind & Breeze (Japan) | 25,360 | 25,360 | 0 |
| Elements Voice Series - Rica Fukami - Private Step (Japan) | 40,564 | 40,564 | 0 |
| Elements Voice Series - Yuri Shiratori - Rainbow Harmony (Japan) | 43,343 | 43,343 | 0 |
| Gamera - The Time Adventure (Japan) | 41,736 | 41,736 | 0 |
| Hello Kitty - Yume no Kuni Daiboken (Japan) | 29,468 | 29,468 | 0 |
| Ie Naki Ko - Suzu no Sentaku (Japan) | 40,649 | 40,649 | 0 |
| Kero Kero Keroppi - Uki Uki Party Land!! (Japan) | 31,642 | 31,642 | 0 |
| Mari-nee no Heya (Japan) | 10,957 | 10,957 | 0 |
| Newton Museum - Kyoryu Nendaiki Kohen (Japan) | 21,683 | 21,683 | 0 |
| Newton Museum - Kyoryu Nendaiki Zenpen (Japan) | 21,939 | 21,939 | 0 |
| Norimono Banzai!! Densha Daishuugou!! (Japan) | 25,694 | 25,694 | 0 |
| Norimono Banzai!! Kuruma Daishuugou!! (Japan) | 27,984 | 27,984 | 0 |
| Playdia - Quick Interactive System - Sample Soft (Japan) | 27,466 | 27,466 | 0 |
| Playdia iQ Kids - Bishoujo Senshi Sailor Moon SuperS - Sailor Moon to Hajimete no Eigo (Japan) | 30,812 | 30,812 | 0 |
| Playdia iQ Kids - Bishoujo Senshi Sailor Moon SuperS - Sailor Moon to Hiragana Lesson! (Japan) | 30,789 | 30,789 | 0 |
| Playdia iQ Kids - Bishoujo Senshi Sailor Moon SuperS - Youkoso! Sailor Youchien (Japan) | 30,400 | 30,400 | 0 |
| Playdia iQ Kids - Nintama Rantarou - Gungun Nobiru Chinou-hen (Japan) | 28,166 | 28,166 | 0 |
| Playdia iQ Kids - Nintama Rantarou - Hajimete Oboeru Chishiki-hen (Japan) | 32,274 | 32,274 | 0 |
| Playdia iQ Kids - Soreike! Anpanman - Picnic de Obenkyou (Japan) | 29,521 | 29,521 | 0 |
| Playdia iQ Kids - Ultraman - Chinou UP Daisakusen (Japan) | 30,355 | 30,355 | 0 |
| Playdia iQ Kids - Ultraman - Hiragana Daisakusen (Japan) | 35,659 | 35,659 | 0 |
| Playdia iQ Kids - Ultraman - Oide yo! Ultra Youchien (Japan) | 32,118 | 32,118 | 0 |
| Playdia iQ Kids - Ultraman - Suuji de Asobou Ultra Land (Japan) | 36,017 | 36,017 | 0 |
| SD Gundam - Daizukan (Japan) | 23,017 | 23,017 | 0 |
| Shuppatsu! Dobutsu Tankentai (Japan) | 33,318 | 33,318 | 0 |
| Ultra Seven - Chikyuu Bouei Sakusen (Japan) | 32,541 | 32,541 | 0 |
| Ultraman - Alphabet TV e Youkoso (Japan) | 27,384 | 27,384 | 0 |
| Ultraman Powered - Kaiju Gekimetsu Sakusen (Japan) | 36,945 | 36,945 | 0 |
| Yumi to Tokoton Playdia (Japan) (Dokidoki Campaign) | 11,283 | 11,283 | 0 |

## Incomplete source packets

Aqua Adventure packet indices 26562, 26563, 26564, 26565, 26599, 26648,
26649 and 26656 exhaust their 12,248-byte payloads in macroblock row 27.
The eight occurrences contain five distinct incomplete payloads, with 5 to 16
blocks unfinished before the terminal marker. Each macroblock row spans eight
image scanlines; at complete-macroblock granularity, the incomplete region is
the bottom-right 8 to 24 pixels by 8 pixels.

All 48 affected F1/F2 sectors have correct Mode2 Form1 EDC and ECC P/Q parity,
supporting source video truncation rather than random sector read corruption.
All failed packets use factor 250, the highest observed on this disc. An
encoder capacity limit is a plausible cause, but remains unproven.

The player rejects incomplete packets, preserves the previous complete picture
and resumes decoding at the next packet. It does not reconstruct missing
coefficients or apply partial-picture concealment. Actual AK8000 error handling
remains unverified and requires an aligned hardware capture or chip trace.
