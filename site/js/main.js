// ============================================================
// PlaydiaEmu — Landing Page Scripts
// i18n + gallery + animations + hero screenshot
// ============================================================

(function () {
  'use strict';

  // ================================================================
  // GAME DATA — Redump titles (from Game-Compatibility.md)
  // Relative paths resolve after pages.yml copies site/* to repo root.
  // ================================================================
  var GAMES = [
    { zh: 'Aqua Adventure - Blue Lilty', en: 'Aqua Adventure - Blue Lilty', img: 'docs/images/Aqua_Adventure_-_Blue_Lilty_Japan.png' },
    { zh: 'Bandai Item Collection 70', en: 'Bandai Item Collection 70', img: 'docs/images/Bandai_Item_Collection_70_Japan.png' },
    { zh: '美少女战士S 问答对决', en: 'Sailor Moon S Quiz Taiketsu', img: 'docs/images/Bishoujo_Senshi_Sailor_Moon_S_-_Quiz_Taiketsu_Sailor_Power_Shuketsu_Japan.png' },
    { zh: '超合金精选', en: 'Chougoukin Selections', img: 'docs/images/Chougoukin_Selections_Japan.png' },
    { zh: '龙珠Z 地球篇', en: 'Dragon Ball Z Chikyuu-hen', img: 'docs/images/Dragon_Ball_Z_-_Shin_Saiyajin_Zetsumetsu_Keikaku_-_Chikyuu-hen_Japan.png' },
    { zh: '龙珠Z 宇宙篇', en: 'Dragon Ball Z Uchuu-hen', img: 'docs/images/Dragon_Ball_Z_-_Shin_Saiyajin_Zetsumetsu_Keikaku_-_Uchuu-hen_Japan.png' },
    { zh: '久川綾 森林摇曳', en: 'Aya Hisakawa - Forest Sways', img: 'docs/images/Elements_Voice_Series_-_Aya_Hisakawa_-_Forest_Sways_Japan.png' },
    { zh: '幸田喜未子 欢迎来到马里科镇', en: 'Mariko Kouda - Marikotown', img: 'docs/images/Elements_Voice_Series_-_Mariko_Kouda_-_Welcome_to_the_Marikotown_Japan.png' },
    { zh: '金田美香 风与微风', en: 'Mika Kanai - Wind & Breeze', img: 'docs/images/Elements_Voice_Series_-_Mika_Kanai_-_Wind_Breeze_Japan.png' },
    { zh: '深见梨加 私人步骤', en: 'Rica Fukami - Private Step', img: 'docs/images/Elements_Voice_Series_-_Rica_Fukami_-_Private_Step_Japan.png' },
    { zh: '白鸟由里 彩虹和谐', en: 'Yuri Shiratori - Rainbow Harmony', img: 'docs/images/Elements_Voice_Series_-_Yuri_Shiratori_-_Rainbow_Harmony_Japan.png' },
    { zh: '加美拉 时间冒险', en: 'Gamera - The Time Adventure', img: 'docs/images/Gamera_-_The_Time_Adventure_Japan.png' },
    { zh: '凯蒂猫 梦之国大冒险', en: 'Hello Kitty - Yume no Kuni', img: 'docs/images/Hello_Kitty_-_Yume_no_Kuni_Daiboken_Japan.png' },
    { zh: '无家可归的孩子 苏兹的选择', en: 'Ie Naki Ko - Suzu no Sentaku', img: 'docs/images/Ie_Naki_Ko_-_Suzu_no_Sentaku_Japan.png' },
    { zh: 'Keroppi 派对乐园', en: 'Keroppi - Party Land', img: 'docs/images/Kero_Kero_Keroppi_-_Uki_Uki_Party_Land_Japan.png' },
    { zh: '玛丽的房间', en: 'Mari-nee no Heya', img: 'docs/images/Mari-nee_no_Heya_Japan.png' },
    { zh: '牛顿博物馆 恐龙纪后篇', en: 'Newton Museum Kohen', img: 'docs/images/Newton_Museum_-_Kyoryu_Nendaiki_Kohen_Japan.png' },
    { zh: '牛顿博物馆 恐龙纪前篇', en: 'Newton Museum Zenpen', img: 'docs/images/Newton_Museum_-_Kyoryu_Nendaiki_Zenpen_Japan.png' },
    { zh: '交通工具万岁 电车大集合', en: 'Densha Daishuugou', img: 'docs/images/Norimono_Banzai_Densha_Daishuugou_Japan.png' },
    { zh: '交通工具万岁 汽车大集合', en: 'Kuruma Daishuugou', img: 'docs/images/Norimono_Banzai_Kuruma_Daishuugou_Japan.png' },
    { zh: 'Sample Soft', en: 'Sample Soft', img: 'docs/images/Playdia_-_Quick_Interactive_System_-_Sample_Soft_Japan.png' },
    { zh: '水手月亮SuperS 英语入门', en: 'Sailor Moon Eigo', img: 'docs/images/Playdia_iQ_Kids_-_Bishoujo_Senshi_Sailor_Moon_SuperS_-_Sailor_Moon_to_Hajimete_no_Eigo_Japan.png' },
    { zh: '水手月亮SuperS 平假名课程', en: 'Sailor Moon Hiragana', img: 'docs/images/Playdia_iQ_Kids_-_Bishoujo_Senshi_Sailor_Moon_SuperS_-_Sailor_Moon_to_Hiragana_Lesson_Japan.png' },
    { zh: '水手月亮SuperS 欢迎幼儿园', en: 'Sailor Youchien', img: 'docs/images/Playdia_iQ_Kids_-_Bishoujo_Senshi_Sailor_Moon_SuperS_-_Youkoso_Sailor_Youchien_Japan.png' },
    { zh: '忍者乱太郎 智力篇', en: 'Nintama Chinou-hen', img: 'docs/images/Playdia_iQ_Kids_-_Nintama_Rantarou_-_Gungun_Nobiru_Chinou-hen_Japan.png' },
    { zh: '忍者乱太郎 知识篇', en: 'Nintama Chishiki-hen', img: 'docs/images/Playdia_iQ_Kids_-_Nintama_Rantarou_-_Hajimete_Oboeru_Chishiki-hen_Japan.png' },
    { zh: '面包超人 野餐学习', en: 'Anpanman Picnic', img: 'docs/images/Playdia_iQ_Kids_-_Soreike_Anpanman_-_Picnic_de_Obenkyou_Japan.png' },
    { zh: '奥特曼 智力UP大作战', en: 'Ultraman Chinou UP', img: 'docs/images/Playdia_iQ_Kids_-_Ultraman_-_Chinou_UP_Daisakusen_Japan.png' },
    { zh: '奥特曼 平假名大作战', en: 'Ultraman Hiragana', img: 'docs/images/Playdia_iQ_Kids_-_Ultraman_-_Hiragana_Daisakusen_Japan.png' },
    { zh: '奥特曼 快来吧幼儿园', en: 'Ultraman Youchien', img: 'docs/images/Playdia_iQ_Kids_-_Ultraman_-_Oide_yo_Ultra_Youchien_Japan.png' },
    { zh: '奥特曼 数字乐园', en: 'Ultraman Ultra Land', img: 'docs/images/Playdia_iQ_Kids_-_Ultraman_-_Suuji_de_Asobou_Ultra_Land_Japan.png' },
    { zh: 'SD高达 大图鉴', en: 'SD Gundam Daizukan', img: 'docs/images/SD_Gundam_-_Daizukan_Japan.png' },
    { zh: '出发！动物坦克探险队', en: 'Shuppatsu! Dobutsu Tankentai', img: 'docs/images/Shuppatsu_Dobutsu_Tankentai_Japan.png' },
    { zh: '赛文奥特曼 地球防卫作战', en: 'Ultra Seven', img: 'docs/images/Ultra_Seven_-_Chikyuu_Bouei_Sakusen_Japan.png' },
    { zh: '奥特曼 字母TV欢迎', en: 'Ultraman Alphabet TV', img: 'docs/images/Ultraman_-_Alphabet_TV_e_Youkoso_Japan.png' },
    { zh: '奥特曼Powered 怪兽歼灭战', en: 'Ultraman Powered', img: 'docs/images/Ultraman_Powered_-_Kaiju_Gekimetsu_Sakusen_Japan.png' },
    { zh: '由美玩到底 Playdia', en: 'Yumi to Tokoton Playdia', img: 'docs/images/Yumi_to_Tokoton_Playdia_Japan_Dokidoki_Campaign.png' }
  ];

  var PAGE_SIZE = 9;
  var currentPage = 0;

  // ================================================================
  // i18n — Translations
  // ================================================================
  var translations = {
    zh: {
      'meta-title': 'PlaydiaEmu — Bandai Playdia 模拟器',
      'meta-desc': '用 Rust 编写的 Bandai Playdia 模拟器，HLE 光盘播放器无需 BIOS，支持 CDS-XA 流、AK8000 视频与 XA ADPCM 音频，提供独立版与 RetroArch 核心。',
      'nav-features': '核心特性',
      'nav-games': '游戏库',
      'nav-arch': '技术架构',
      'nav-quickstart': '快速开始',
      'hero-subtitle': '让光盘时代的互动体验在现代设备上重生',
      'hero-desc': '用 Rust 编写的 Bandai Playdia 模拟器，通过 HLE 光盘播放器直接播放真实 CDS-XA 内容，无需 BIOS',
      'hero-download': '下载',
      'hero-github': '查看源码',
      'hero-scroll': '向下滚动探索',
      'hero-shot-note': 'PlaydiaEmu 实际运行画面',
      'about-title': '什么是 Playdia？',
      'about-p1': 'Playdia 是万代在 1990 年代推出的日本互动 CD 主机。软件以全动态视频为主，通过 <strong>CD-XA</strong> 双轨光盘流把音频与视频送到解码板。',
      'about-p2': 'PlaydiaEmu 用 HLE 光盘播放器直接解码真实光盘内容——CUE/BIN 或 Redump 风格 ZIP 都可加载，无需 BIOS。',
      'about-disc': 'CDS-XA 光盘',
      'about-disc-sub': '双轨 MODE2 CUE/BIN',
      'about-stream': 'F1 / F2 / F3 流',
      'about-stream-sub': '视频标记 + XA 音频',
      'about-emu-sub': 'Rust 模拟器',
      'stat-games': '兼容游戏',
      'stat-games-sub': '批处理截图全部出图',
      'stat-bios': 'BIOS 需求',
      'stat-bios-sub': 'HLE 播放器开箱即用',
      'stat-frontends': '运行前端',
      'stat-frontends-sub': 'Standalone + RetroArch',
      'stat-display': '输出画面',
      'stat-display-sub': '320×240 · XA ADPCM 44.1 kHz',
      'feat-title': '核心特性',
      'feat-subtitle': '从光盘流到窗口与 RetroArch，全栈 Rust 实现',
      'feat-player-title': 'HLE 光盘播放器',
      'feat-player-desc': '双轨 MODE2 CUE/BIN 流式播放，支持 Redump 风格 ZIP 直接加载，无需 BIOS。',
      'feat-routing-title': 'F1 / F2 / F3 路由',
      'feat-routing-desc': '视频片段组装、F2 溢出与场景跳转、F3 填充，以及按键选择交互控制。',
      'feat-video-title': 'AK8000 游戏视频',
      'feat-video-desc': '恢复的行标记 / VLC 解码产出可识别的 248×216 画面，居中于 320×240 XRGB8888 帧缓冲。',
      'feat-audio-title': 'XA ADPCM 音频',
      'feat-audio-desc': 'Green Book CD-XA 4-bit ADPCM，37800 / 18900 Hz 解码并重采样为 44100 Hz 立体声。',
      'feat-state-title': '即时存档',
      'feat-state-desc': '光盘身份 + CRC 信封，版本 4 保存完整的 HLE 播放状态。',
      'feat-retro-title': 'RetroArch 核心',
      'feat-retro-desc': '完整的 libretro 核心：RetroPad 映射、核心选项、即时存档，与独立版共享同一播放器。',
      'gallery-title': '游戏库',
      'gallery-subtitle': '37 款 Redump 标题均已完成批处理截图验证',
      'gallery-more': '查看完整兼容性列表 →',
      'arch-title': '技术架构',
      'arch-subtitle': '平台无关的核心引擎，双前端共享同一套播放逻辑',
      'arch-frontends': '前端',
      'arch-standalone-sub': 'Standalone 可执行文件<br>窗口 / --headless',
      'arch-libretro-sub': 'libretro cdylib<br>RetroArch 核心',
      'arch-core': '核心引擎',
      'arch-core-sub': '平台无关的库 · 无宿主 I/O',
      'arch-platforms': '目标平台',
      'qs-title': '快速开始',
      'qs-subtitle': '几行命令，即刻体验',
      'qs-standalone': 'Standalone',
      'qs-standalone-1': '下载最新版本',
      'qs-standalone-1-sub': '从 Releases 页面下载对应平台的二进制文件',
      'qs-standalone-2': '运行游戏',
      'qs-standalone-3': '或直接加载 ZIP',
      'qs-retro-1': '下载 libretro 核心',
      'qs-retro-1-sub': '从 Releases 页面下载对应平台的核心文件',
      'qs-retro-2': '安装核心',
      'qs-retro-2-sub': '复制到 RetroArch 的 cores/ 目录',
      'qs-retro-3': '加载核心并启动',
      'qs-build': '从源码编译',
      'qs-build-1': '克隆仓库',
      'qs-build-2': '编译 Standalone',
      'qs-build-3': '或编译 RetroArch 核心',
      'footer-desc': '用 Rust 编写的 Bandai Playdia 模拟器',
      'footer-project': '项目',
      'footer-contributing': '贡献指南',
      'footer-community': '社区',
      'footer-docs': '文档',
      'footer-cli': '独立模拟器',
      'footer-core': 'RetroArch Core',
      'footer-gamelist': '游戏兼容性',
      'footer-copy': 'BSD 3-Clause License &copy; 2026 Aloys. Built with 🦀 Rust.'
    },
    en: {
      'meta-title': 'PlaydiaEmu — Bandai Playdia Emulator',
      'meta-desc': 'A Rust emulator for the Bandai Playdia. The HLE disc player needs no BIOS and handles CDS-XA streams, AK8000 video, and XA ADPCM audio, with standalone and RetroArch frontends.',
      'nav-features': 'Features',
      'nav-games': 'Games',
      'nav-arch': 'Architecture',
      'nav-quickstart': 'Quick Start',
      'hero-subtitle': 'Bring the disc-era interactive experience back to modern devices',
      'hero-desc': 'A Rust emulator for the Bandai Playdia that plays real CDS-XA content through an HLE disc player — no BIOS required',
      'hero-download': 'Download',
      'hero-github': 'View Source',
      'hero-scroll': 'Scroll to explore',
      'hero-shot-note': 'Actual PlaydiaEmu output',
      'about-title': 'What is Playdia?',
      'about-p1': 'Playdia is a 1990s Japanese interactive CD console from Bandai. Titles are largely full-motion video driven by a <strong>CD-XA</strong> dual-track disc stream delivered to a co-processor board.',
      'about-p2': 'PlaydiaEmu decodes real disc content through an HLE disc player — load CUE/BIN or Redump-style ZIPs with no BIOS.',
      'about-disc': 'CDS-XA Disc',
      'about-disc-sub': 'Dual-track MODE2 CUE/BIN',
      'about-stream': 'F1 / F2 / F3 Stream',
      'about-stream-sub': 'Video markers + XA audio',
      'about-emu-sub': 'Rust Emulator',
      'stat-games': 'Compatible Games',
      'stat-games-sub': 'All batch screenshots non-blank',
      'stat-bios': 'BIOS Required',
      'stat-bios-sub': 'HLE player works out of the box',
      'stat-frontends': 'Frontends',
      'stat-frontends-sub': 'Standalone + RetroArch',
      'stat-display': 'Output',
      'stat-display-sub': '320×240 · XA ADPCM 44.1 kHz',
      'feat-title': 'Core Features',
      'feat-subtitle': 'From disc streams to window and RetroArch — all in Rust',
      'feat-player-title': 'HLE Disc Player',
      'feat-player-desc': 'Dual-track MODE2 CUE/BIN streaming with Redump-style ZIP loading and no BIOS required.',
      'feat-routing-title': 'F1 / F2 / F3 Routing',
      'feat-routing-desc': 'Video fragment assembly, F2 overflow and scene jumps, F3 padding, and button-choice interactivity.',
      'feat-video-title': 'AK8000 Game Video',
      'feat-video-desc': 'Recovered row-marker / VLC decoding produces recognizable 248×216 pictures centered in a 320×240 XRGB8888 framebuffer.',
      'feat-audio-title': 'XA ADPCM Audio',
      'feat-audio-desc': 'Green Book CD-XA 4-bit ADPCM at 37800 / 18900 Hz, resampled to 44100 Hz stereo.',
      'feat-state-title': 'Save States',
      'feat-state-desc': 'Disc identity + CRC envelope; version 4 stores the complete HLE playback state.',
      'feat-retro-title': 'RetroArch Core',
      'feat-retro-desc': 'A full libretro core with RetroPad mapping, core options, and save states — same player as standalone.',
      'gallery-title': 'Game Library',
      'gallery-subtitle': '37 Redump titles verified with batch screenshots',
      'gallery-more': 'See the full compatibility list →',
      'arch-title': 'Architecture',
      'arch-subtitle': 'A platform-independent core shared by two frontends',
      'arch-frontends': 'Frontends',
      'arch-standalone-sub': 'Standalone binary<br>window / --headless',
      'arch-libretro-sub': 'libretro cdylib<br>RetroArch core',
      'arch-core': 'Core Engine',
      'arch-core-sub': 'Platform-independent library · no host I/O',
      'arch-platforms': 'Target Platforms',
      'qs-title': 'Quick Start',
      'qs-subtitle': 'A few commands and you are playing',
      'qs-standalone': 'Standalone',
      'qs-standalone-1': 'Download the latest release',
      'qs-standalone-1-sub': 'Grab the binary for your platform from Releases',
      'qs-standalone-2': 'Run a game',
      'qs-standalone-3': 'Or load a ZIP directly',
      'qs-retro-1': 'Download the libretro core',
      'qs-retro-1-sub': 'Grab the core file for your platform from Releases',
      'qs-retro-2': 'Install the core',
      'qs-retro-2-sub': 'Copy it into RetroArch cores/',
      'qs-retro-3': 'Load core and start',
      'qs-build': 'Build from Source',
      'qs-build-1': 'Clone the repo',
      'qs-build-2': 'Build Standalone',
      'qs-build-3': 'Or build the RetroArch core',
      'footer-desc': 'A Rust emulator for the Bandai Playdia',
      'footer-project': 'Project',
      'footer-contributing': 'Contributing',
      'footer-community': 'Community',
      'footer-docs': 'Docs',
      'footer-cli': 'Standalone Emulator',
      'footer-core': 'RetroArch Core',
      'footer-gamelist': 'Game Compatibility',
      'footer-copy': 'BSD 3-Clause License &copy; 2026 Aloys. Built with 🦀 Rust.'
    }
  };

  var currentLang = localStorage.getItem('playdia-site-lang') || 'zh';

  function applyLang(lang) {
    document.documentElement.lang = lang === 'zh' ? 'zh-CN' : 'en';
    if (document.title) {
      document.title = translations[lang]['meta-title'];
    }
    var meta = document.querySelector('meta[name="description"]');
    if (meta) {
      meta.setAttribute('content', translations[lang]['meta-desc']);
    }
    var toggle = document.getElementById('lang-toggle');
    if (toggle) {
      toggle.textContent = lang === 'zh' ? 'EN' : '中文';
    }
    document.querySelectorAll('[data-i18n]').forEach(function (node) {
      var key = node.getAttribute('data-i18n');
      if (translations[lang][key] != null) {
        node.innerHTML = translations[lang][key];
      }
    });
    buildGallery();
    updateHeroShot();
  }

  function updateHeroShot() {
    var img = document.getElementById('hero-shot');
    if (!img) return;
    var pick = GAMES[20] || GAMES[0];
    img.src = pick.img;
    img.alt = 'PlaydiaEmu running ' + pick[currentLang];
  }

  function buildGallery() {
    var root = document.getElementById('gallery-dynamic');
    if (!root) return;

    var totalPages = Math.max(1, Math.ceil(GAMES.length / PAGE_SIZE));
    if (currentPage >= totalPages) currentPage = 0;

    var pageGames = GAMES.slice(currentPage * PAGE_SIZE, (currentPage + 1) * PAGE_SIZE);
    var pageHtml = pageGames.map(function (game) {
      var title = game[currentLang] || game.en;
      return (
        '<figure class="gallery-item">' +
        '<img loading="lazy" src="' + game.img + '" alt="' + title.replace(/"/g, '&quot;') + '">' +
        '<figcaption>' + title + '</figcaption>' +
        '</figure>'
      );
    }).join('');

    var dots = '';
    for (var i = 0; i < totalPages; i++) {
      dots += '<span data-page="' + i + '" class="' + (i === currentPage ? 'active' : '') + '"></span>';
    }

    root.innerHTML =
      '<div class="gallery-pages"><div class="gallery-page active">' + pageHtml + '</div></div>' +
      '<div class="gallery-nav">' +
      '<button type="button" data-gallery-prev aria-label="Previous" ' + (currentPage === 0 ? 'disabled' : '') + '>‹</button>' +
      '<div class="gallery-dots">' + dots + '</div>' +
      '<button type="button" data-gallery-next aria-label="Next" ' + (currentPage >= totalPages - 1 ? 'disabled' : '') + '>›</button>' +
      '</div>';

    var prev = root.querySelector('[data-gallery-prev]');
    var next = root.querySelector('[data-gallery-next]');
    if (prev) {
      prev.addEventListener('click', function () {
        if (currentPage > 0) {
          currentPage -= 1;
          buildGallery();
        }
      });
    }
    if (next) {
      next.addEventListener('click', function () {
        if (currentPage < totalPages - 1) {
          currentPage += 1;
          buildGallery();
        }
      });
    }
    root.querySelectorAll('.gallery-dots span').forEach(function (dot) {
      dot.addEventListener('click', function () {
        currentPage = parseInt(dot.getAttribute('data-page'), 10) || 0;
        buildGallery();
      });
    });
  }

  // ================================================================
  // NAVBAR — Scroll effect + mobile toggle
  // ================================================================
  var navbar = document.getElementById('navbar');

  function onScroll() {
    if (navbar) navbar.classList.toggle('scrolled', window.scrollY > 50);
  }

  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();

  var navToggle = document.querySelector('.nav-toggle');
  var navLinks = document.querySelector('.nav-links');

  if (navToggle && navLinks) {
    navToggle.addEventListener('click', function () {
      navLinks.classList.toggle('open');
    });
    navLinks.querySelectorAll('a').forEach(function (a) {
      a.addEventListener('click', function () { navLinks.classList.remove('open'); });
    });
  }

  // ================================================================
  // SCROLL REVEAL
  // ================================================================
  var fadeEls = document.querySelectorAll('.fade-in-up');

  if ('IntersectionObserver' in window) {
    var observer = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting) {
          entry.target.classList.add('visible');
          observer.unobserve(entry.target);
        }
      });
    }, { threshold: 0.1, rootMargin: '0px 0px -40px 0px' });

    fadeEls.forEach(function (el) { observer.observe(el); });
  } else {
    fadeEls.forEach(function (el) { el.classList.add('visible'); });
  }

  // ================================================================
  // ANIMATED COUNTER
  // ================================================================
  var statNumbers = document.querySelectorAll('.stat-number[data-target]');

  function animateCounter(el) {
    var target = parseInt(el.getAttribute('data-target'), 10);
    var suffix = el.getAttribute('data-suffix') || '';
    var duration = 1600;
    var start = performance.now();

    function tick(now) {
      var progress = Math.min((now - start) / duration, 1);
      var eased = 1 - Math.pow(1 - progress, 3);
      el.textContent = Math.round(eased * target) + suffix;
      if (progress < 1) requestAnimationFrame(tick);
    }

    requestAnimationFrame(tick);
  }

  if ('IntersectionObserver' in window) {
    var statObserver = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting) {
          animateCounter(entry.target);
          statObserver.unobserve(entry.target);
        }
      });
    }, { threshold: 0.5 });
    statNumbers.forEach(function (el) { statObserver.observe(el); });
  } else {
    statNumbers.forEach(function (el) { animateCounter(el); });
  }

  // ================================================================
  // DISC CANVAS — Soft concentric CD rings
  // ================================================================
  var canvas = document.getElementById('disc-canvas');
  if (canvas && canvas.getContext) {
    var ctx = canvas.getContext('2d');
    var w = 0;
    var h = 0;
    var angle = 0;

    function resize() {
      w = canvas.width = canvas.offsetWidth;
      h = canvas.height = canvas.offsetHeight;
    }

    function draw() {
      ctx.clearRect(0, 0, w, h);
      var cx = w / 2;
      var cy = h / 2;
      var maxR = Math.min(w, h) * 0.42;

      for (var r = maxR; r > 18; r -= 14) {
        ctx.beginPath();
        ctx.arc(cx, cy, r, 0, Math.PI * 2);
        ctx.strokeStyle = r % 28 < 14 ? 'rgba(232, 85, 58, 0.35)' : 'rgba(79, 209, 197, 0.25)';
        ctx.lineWidth = 1;
        ctx.stroke();
      }

      // Highlight sweep
      var sweep = angle;
      var grad = ctx.createConicGradient
        ? ctx.createConicGradient(sweep, cx, cy)
        : null;
      if (grad) {
        grad.addColorStop(0, 'rgba(232, 85, 58, 0)');
        grad.addColorStop(0.15, 'rgba(232, 85, 58, 0.55)');
        grad.addColorStop(0.3, 'rgba(79, 209, 197, 0)');
        grad.addColorStop(1, 'rgba(79, 209, 197, 0)');
        ctx.beginPath();
        ctx.arc(cx, cy, maxR, 0, Math.PI * 2);
        ctx.fillStyle = grad;
        ctx.fill();
      }

      // Center hole
      ctx.beginPath();
      ctx.arc(cx, cy, 16, 0, Math.PI * 2);
      ctx.fillStyle = 'rgba(10, 14, 20, 0.9)';
      ctx.fill();
      ctx.strokeStyle = 'rgba(232, 85, 58, 0.5)';
      ctx.stroke();

      angle += 0.012;
      requestAnimationFrame(draw);
    }

    resize();
    draw();
    window.addEventListener('resize', resize);
  }

  // ================================================================
  // SMOOTH SCROLL
  // ================================================================
  document.querySelectorAll('a[href^="#"]').forEach(function (anchor) {
    anchor.addEventListener('click', function (e) {
      var target = document.querySelector(anchor.getAttribute('href'));
      if (target) {
        e.preventDefault();
        target.scrollIntoView({ behavior: 'smooth', block: 'start' });
      }
    });
  });

  // ================================================================
  // LANGUAGE TOGGLE
  // ================================================================
  var langBtn = document.getElementById('lang-toggle');
  if (langBtn) {
    langBtn.addEventListener('click', function () {
      currentLang = currentLang === 'zh' ? 'en' : 'zh';
      localStorage.setItem('playdia-site-lang', currentLang);
      applyLang(currentLang);
    });
  }

  // ================================================================
  // INIT
  // ================================================================
  applyLang(currentLang);
})();
