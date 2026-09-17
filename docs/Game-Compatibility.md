# Game Compatibility

HLE player (no BIOS). Counts below cover batch screenshots, deeper integration
checks, and structural entropy validation — not full playthroughs.

## Summary

| Status | Count |
|--------|-------|
| ✅ Batch screenshot (non-blank frame rendered) | 37 |
| HLE player integration checks (load + stream + video + audio) | 2 |
| Native entropy validation across complete disc streams | 37 |

## Batch screenshots (2026-09-16)

All 37 Redump ZIP titles completed `scripts/batch-screenshots.ps1` against the
release `playdia-emu` binary and wrote a non-blank PNG under `docs/images/`.
Capture uses 300 host frames (10 s at 30 fps) unless overridden: Aqua
Adventure 360 frames; Mari-nee no Heya and Sample Soft 180 frames; Dragon Ball
Z Chikyuu-hen 600 frames. Titles that hold on the BANDAI boot logo
(Chougoukin Selections, Gamera, Hello Kitty, both Nintama Rantarou discs) also
receive scripted `--press-at` A presses so the capture leaves the logo.
ZIPs are loaded in place (CUE+BIN inside the archive) — no extraction step.

These frames establish startup / early demo rendering only. They do not prove
navigation completeness, audio quality on host sinks, save data, full
playthrough, or hardware pixel accuracy. Two titles additionally have deeper
HLE player integration checks: Mari-nee no Heya (native title/button screen;
load, stream, video, audio) and Sample Soft (native menu and demo imagery;
load, stream, video, audio).

## Game List

| # | Title | Screenshot | Status |
|---|-------|------------|--------|
| 1 | Aqua Adventure - Blue Lilty (Japan) | ![Aqua Adventure](images/Aqua_Adventure_-_Blue_Lilty_Japan.png) | ✅ Screenshot |
| 2 | Bandai Item Collection 70' (Japan) | ![Bandai Item Collection 70](images/Bandai_Item_Collection_70_Japan.png) | ✅ Screenshot |
| 3 | Bishoujo Senshi Sailor Moon S - Quiz Taiketsu! Sailor Power Shuketsu!! (Japan) | ![Sailor Moon S Quiz](images/Bishoujo_Senshi_Sailor_Moon_S_-_Quiz_Taiketsu_Sailor_Power_Shuketsu_Japan.png) | ✅ Screenshot |
| 4 | Chougoukin Selections (Japan) | ![Chougoukin Selections](images/Chougoukin_Selections_Japan.png) | ✅ Screenshot |
| 5 | Dragon Ball Z - Shin Saiyajin Zetsumetsu Keikaku - Chikyuu-hen (Japan) | ![DBZ Chikyuu-hen](images/Dragon_Ball_Z_-_Shin_Saiyajin_Zetsumetsu_Keikaku_-_Chikyuu-hen_Japan.png) | ✅ Screenshot |
| 6 | Dragon Ball Z - Shin Saiyajin Zetsumetsu Keikaku - Uchuu-hen (Japan) | ![DBZ Uchuu-hen](images/Dragon_Ball_Z_-_Shin_Saiyajin_Zetsumetsu_Keikaku_-_Uchuu-hen_Japan.png) | ✅ Screenshot |
| 7 | Elements Voice Series - Aya Hisakawa - Forest Sways (Japan) | ![Forest Sways](images/Elements_Voice_Series_-_Aya_Hisakawa_-_Forest_Sways_Japan.png) | ✅ Screenshot |
| 8 | Elements Voice Series - Mariko Kouda - Welcome to the Marikotown! (Japan) | ![Marikotown](images/Elements_Voice_Series_-_Mariko_Kouda_-_Welcome_to_the_Marikotown_Japan.png) | ✅ Screenshot |
| 9 | Elements Voice Series - Mika Kanai - Wind & Breeze (Japan) | ![Wind and Breeze](images/Elements_Voice_Series_-_Mika_Kanai_-_Wind_Breeze_Japan.png) | ✅ Screenshot |
| 10 | Elements Voice Series - Rica Fukami - Private Step (Japan) | ![Private Step](images/Elements_Voice_Series_-_Rica_Fukami_-_Private_Step_Japan.png) | ✅ Screenshot |
| 11 | Elements Voice Series - Yuri Shiratori - Rainbow Harmony (Japan) | ![Rainbow Harmony](images/Elements_Voice_Series_-_Yuri_Shiratori_-_Rainbow_Harmony_Japan.png) | ✅ Screenshot |
| 12 | Gamera - The Time Adventure (Japan) | ![Gamera](images/Gamera_-_The_Time_Adventure_Japan.png) | ✅ Screenshot |
| 13 | Hello Kitty - Yume no Kuni Daiboken (Japan) | ![Hello Kitty](images/Hello_Kitty_-_Yume_no_Kuni_Daiboken_Japan.png) | ✅ Screenshot |
| 14 | Ie Naki Ko - Suzu no Sentaku (Japan) | ![Ie Naki Ko](images/Ie_Naki_Ko_-_Suzu_no_Sentaku_Japan.png) | ✅ Screenshot |
| 15 | Kero Kero Keroppi - Uki Uki Party Land!! (Japan) | ![Keroppi](images/Kero_Kero_Keroppi_-_Uki_Uki_Party_Land_Japan.png) | ✅ Screenshot |
| 16 | Mari-nee no Heya (Japan) | ![Mari-nee no Heya](images/Mari-nee_no_Heya_Japan.png) | ✅ Screenshot + HLE check |
| 17 | Newton Museum - Kyoryu Nendaiki Kohen (Japan) | ![Newton Museum Kohen](images/Newton_Museum_-_Kyoryu_Nendaiki_Kohen_Japan.png) | ✅ Screenshot |
| 18 | Newton Museum - Kyoryu Nendaiki Zenpen (Japan) | ![Newton Museum Zenpen](images/Newton_Museum_-_Kyoryu_Nendaiki_Zenpen_Japan.png) | ✅ Screenshot |
| 19 | Norimono Banzai!! Densha Daishuugou!! (Japan) | ![Densha Daishuugou](images/Norimono_Banzai_Densha_Daishuugou_Japan.png) | ✅ Screenshot |
| 20 | Norimono Banzai!! Kuruma Daishuugou!! (Japan) | ![Kuruma Daishuugou](images/Norimono_Banzai_Kuruma_Daishuugou_Japan.png) | ✅ Screenshot |
| 21 | Playdia - Quick Interactive System - Sample Soft (Japan) | ![Sample Soft](images/Playdia_-_Quick_Interactive_System_-_Sample_Soft_Japan.png) | ✅ Screenshot + HLE check |
| 22 | Playdia iQ Kids - Bishoujo Senshi Sailor Moon SuperS - Sailor Moon to Hajimete no Eigo (Japan) | ![Sailor Moon Eigo](images/Playdia_iQ_Kids_-_Bishoujo_Senshi_Sailor_Moon_SuperS_-_Sailor_Moon_to_Hajimete_no_Eigo_Japan.png) | ✅ Screenshot |
| 23 | Playdia iQ Kids - Bishoujo Senshi Sailor Moon SuperS - Sailor Moon to Hiragana Lesson! (Japan) | ![Sailor Moon Hiragana](images/Playdia_iQ_Kids_-_Bishoujo_Senshi_Sailor_Moon_SuperS_-_Sailor_Moon_to_Hiragana_Lesson_Japan.png) | ✅ Screenshot |
| 24 | Playdia iQ Kids - Bishoujo Senshi Sailor Moon SuperS - Youkoso! Sailor Youchien (Japan) | ![Sailor Youchien](images/Playdia_iQ_Kids_-_Bishoujo_Senshi_Sailor_Moon_SuperS_-_Youkoso_Sailor_Youchien_Japan.png) | ✅ Screenshot |
| 25 | Playdia iQ Kids - Nintama Rantarou - Gungun Nobiru Chinou-hen (Japan) | ![Nintama Chinou](images/Playdia_iQ_Kids_-_Nintama_Rantarou_-_Gungun_Nobiru_Chinou-hen_Japan.png) | ✅ Screenshot |
| 26 | Playdia iQ Kids - Nintama Rantarou - Hajimete Oboeru Chishiki-hen (Japan) | ![Nintama Chishiki](images/Playdia_iQ_Kids_-_Nintama_Rantarou_-_Hajimete_Oboeru_Chishiki-hen_Japan.png) | ✅ Screenshot |
| 27 | Playdia iQ Kids - Soreike! Anpanman - Picnic de Obenkyou (Japan) | ![Anpanman](images/Playdia_iQ_Kids_-_Soreike_Anpanman_-_Picnic_de_Obenkyou_Japan.png) | ✅ Screenshot |
| 28 | Playdia iQ Kids - Ultraman - Chinou UP Daisakusen (Japan) | ![Ultraman Chinou UP](images/Playdia_iQ_Kids_-_Ultraman_-_Chinou_UP_Daisakusen_Japan.png) | ✅ Screenshot |
| 29 | Playdia iQ Kids - Ultraman - Hiragana Daisakusen (Japan) | ![Ultraman Hiragana](images/Playdia_iQ_Kids_-_Ultraman_-_Hiragana_Daisakusen_Japan.png) | ✅ Screenshot |
| 30 | Playdia iQ Kids - Ultraman - Oide yo! Ultra Youchien (Japan) | ![Ultraman Youchien](images/Playdia_iQ_Kids_-_Ultraman_-_Oide_yo_Ultra_Youchien_Japan.png) | ✅ Screenshot |
| 31 | Playdia iQ Kids - Ultraman - Suuji de Asobou Ultra Land (Japan) | ![Ultraman Ultra Land](images/Playdia_iQ_Kids_-_Ultraman_-_Suuji_de_Asobou_Ultra_Land_Japan.png) | ✅ Screenshot |
| 32 | SD Gundam - Daizukan (Japan) | ![SD Gundam](images/SD_Gundam_-_Daizukan_Japan.png) | ✅ Screenshot |
| 33 | Shuppatsu! Dobutsu Tankentai (Japan) | ![Dobutsu Tankentai](images/Shuppatsu_Dobutsu_Tankentai_Japan.png) | ✅ Screenshot |
| 34 | Ultra Seven - Chikyuu Bouei Sakusen (Japan) | ![Ultra Seven](images/Ultra_Seven_-_Chikyuu_Bouei_Sakusen_Japan.png) | ✅ Screenshot |
| 35 | Ultraman - Alphabet TV e Youkoso (Japan) | ![Ultraman Alphabet TV](images/Ultraman_-_Alphabet_TV_e_Youkoso_Japan.png) | ✅ Screenshot |
| 36 | Ultraman Powered - Kaiju Gekimetsu Sakusen (Japan) | ![Ultraman Powered](images/Ultraman_Powered_-_Kaiju_Gekimetsu_Sakusen_Japan.png) | ✅ Screenshot |
| 37 | Yumi to Tokoton Playdia (Japan) (Dokidoki Campaign) | ![Yumi to Tokoton](images/Yumi_to_Tokoton_Playdia_Japan_Dokidoki_Campaign.png) | ✅ Screenshot |

Legend:

| Symbol | Meaning |
|--------|---------|
| ✅ Screenshot | Batch capture produced a non-blank early frame (startup / title / demo). |
| ✅ Screenshot + HLE check | Screenshot plus load / stream / video / audio integration notes above. |
| ❌ Fail | Crash, timeout, or no usable screenshot. |

## How to update

Regenerate the published matrix (ZIPs in place; private path is not committed):

```powershell
.\scripts\batch-screenshots.ps1 -RedumpDir <local-redump-folder>
```

Inspect a title:

```powershell
cargo run --release -p playdiaemu-tools --bin playdia-inspect -- "path/to/title.cue"
```

Play headlessly and dump a frame:

```powershell
cargo run --release -p playdiaemu -- `
  "path/to/title.cue" `
  --headless --frames 120
```

Record:

1. Whether the CUE / ZIP loads
2. Stream track number and F1/F2/F3 / audio sector counts from `playdia-inspect`
3. Host frames run and whether the PPM / PNG is non-blank
4. Approximate PCM sample count from logs

## Notes

- Default video uses the recovered **248×216 AK8000 decoder**. Sample Soft,
  Dragon Ball Z and Mari-nee pictures are recognizable; this does not establish
  hardware pixel accuracy or full game compatibility. Use `playdia-frame
  --check-all` to measure entropy coverage separately from navigation.
- F2 jump and button-choice navigation is exercised on a private disc; timeout,
  quiz, and score semantics have not been verified on hardware.
- Prefer `--release` builds for any visual check.
- Do not commit discs, BIOS, or PPM dumps. Screenshots under `docs/images/`
  are PNG previews only.
