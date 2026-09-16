//! libretro API implementation.

#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::chunks_exact_to_as_chunks)]

use std::ffi::{c_void, CStr};
use std::os::raw::{c_char, c_int, c_uint};
use std::ptr;
use std::sync::atomic::{AtomicU8, Ordering};

use playdiaemu_core::{InputButtons, FB_HEIGHT, FB_WIDTH};

use crate::callbacks;
use crate::constants::*;
use crate::logger;
use crate::types::*;
use crate::CORE;

static VOLUME: AtomicU8 = AtomicU8::new(100);
static SWAP_AB: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn cstr(bytes: &[u8]) -> *const c_char {
    bytes.as_ptr() as *const c_char
}

// ============================================================
// Startup / callbacks
// ============================================================

#[no_mangle]
pub extern "C" fn retro_set_environment(callback: RetroEnvironmentCallback) {
    callbacks::set_environment(callback);
    set_core_options();
}

#[no_mangle]
pub extern "C" fn retro_set_video_refresh(callback: RetroVideoRefreshCallback) {
    callbacks::set_video_refresh(callback);
}

#[no_mangle]
pub extern "C" fn retro_set_audio_sample(callback: RetroAudioSampleCallback) {
    callbacks::set_audio_sample(callback);
}

#[no_mangle]
pub extern "C" fn retro_set_audio_sample_batch(callback: RetroAudioSampleBatchCallback) {
    callbacks::set_audio_sample_batch(callback);
}

#[no_mangle]
pub extern "C" fn retro_set_input_poll(callback: RetroInputPollCallback) {
    callbacks::set_input_poll(callback);
}

#[no_mangle]
pub extern "C" fn retro_set_input_state(callback: RetroInputStateCallback) {
    callbacks::set_input_state(callback);
}

#[no_mangle]
pub extern "C" fn retro_api_version() -> c_uint {
    RETRO_API_VERSION as c_uint
}

#[no_mangle]
pub extern "C" fn retro_init() {
    callbacks::initialize_log_interface();
    logger::initialize();
    log::info!("Libretro core initialized");
}

#[no_mangle]
pub extern "C" fn retro_deinit() {
    *CORE.lock().unwrap() = None;
    logger::set_debug_logging(false);
    log::info!("Libretro core deinitialized");
}

#[no_mangle]
pub extern "C" fn retro_get_system_info(info: *mut RetroSystemInfo) {
    let Some(info) = (unsafe { info.as_mut() }) else {
        return;
    };
    info.library_name = c"PlaydiaEmu".as_ptr();
    info.library_version = c"0.1.0".as_ptr();
    info.valid_extensions = c"cue|zip|iso|bin".as_ptr();
    info.need_fullpath = true;
    // Pass ZIP paths to the core; do not let the frontend extract them.
    info.block_extract = true;
}

#[no_mangle]
pub extern "C" fn retro_get_system_av_info(info: *mut RetroSystemAvInfo) {
    let Some(info) = (unsafe { info.as_mut() }) else {
        return;
    };
    info.geometry = RetroGameGeometry {
        base_width: FB_WIDTH as c_uint,
        base_height: FB_HEIGHT as c_uint,
        max_width: FB_WIDTH as c_uint,
        max_height: FB_HEIGHT as c_uint,
        aspect_ratio: 4.0 / 3.0,
    };
    info.timing = RetroSystemTiming {
        fps: FRAMES_PER_SECOND,
        sample_rate: OUTPUT_SAMPLE_RATE,
    };
}

#[no_mangle]
pub extern "C" fn retro_set_controller_port_device(port: c_uint, device: c_uint) {
    if port == 0 && device != 0 && device != RETRO_DEVICE_JOYPAD {
        log::warn!("Unsupported controller device {device} on port {port}");
    }
}

#[no_mangle]
pub extern "C" fn retro_get_region() -> c_uint {
    RETRO_REGION_NTSC
}

// ============================================================
// Load / run
// ============================================================

#[no_mangle]
pub extern "C" fn retro_load_game(info: *const RetroGameInfo) -> bool {
    let Some(info) = (unsafe { info.as_ref() }) else {
        return false;
    };
    if info.path.is_null() && (info.data.is_null() || info.size == 0) {
        return false;
    }

    let mut player = playdiaemu_core::player::DiscPlayer::new();
    if !info.path.is_null() {
        let path = unsafe {
            let mut len = 0usize;
            while *info.path.add(len) != 0 {
                len += 1;
            }
            std::slice::from_raw_parts(info.path as *const u8, len)
        };
        let Ok(s) = std::str::from_utf8(path) else {
            return false;
        };
        if player.load_path(std::path::Path::new(s)).is_err() {
            return false;
        }
    } else {
        let bytes =
            unsafe { std::slice::from_raw_parts(info.data as *const u8, info.size).to_vec() };
        if player.load_bytes(bytes).is_err() {
            return false;
        }
    }

    if !set_pixel_format() {
        log::error!("Frontend rejected the required XRGB8888 pixel format");
        return false;
    }
    register_input_descriptors();
    set_performance_level();
    apply_core_options();
    *CORE.lock().unwrap() = Some(player);
    log::info!("Loaded HLE disc player content");
    true
}

#[no_mangle]
pub extern "C" fn retro_load_game_special(
    _type: c_uint,
    _info: *const RetroGameInfo,
    _num: usize,
) -> bool {
    false
}

#[no_mangle]
pub extern "C" fn retro_unload_game() {
    *CORE.lock().unwrap() = None;
    log::info!("Game unloaded");
}

#[no_mangle]
pub extern "C" fn retro_reset() {
    if let Ok(mut guard) = CORE.lock() {
        if let Some(player) = guard.as_mut() {
            player.reset();
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_run() {
    if core_options_changed() {
        apply_core_options();
    }

    poll_input();

    let Ok(mut guard) = CORE.lock() else {
        return;
    };
    let Some(player) = guard.as_mut() else {
        return;
    };
    let _ = player.run_frame();
    let fb = player.framebuffer();
    callbacks::video_refresh(
        fb.as_ptr() as *const c_void,
        FB_WIDTH as c_uint,
        FB_HEIGHT as c_uint,
        FB_WIDTH * 4,
    );

    let mut audio = player.drain_audio();
    if !audio.is_empty() {
        scale_volume(&mut audio, VOLUME.load(Ordering::Relaxed));
        if callbacks::audio_sample_batch(audio.as_ptr(), audio.len() / 2).is_none() {
            for pair in audio.chunks_exact(2) {
                callbacks::audio_sample(pair[0], pair[1]);
            }
        }
    }
}

// ============================================================
// Save state / cheats / memory
// ============================================================

#[no_mangle]
pub extern "C" fn retro_serialize_size() -> usize {
    if let Ok(guard) = CORE.lock() {
        if let Some(player) = guard.as_ref() {
            return player.save_state().len();
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn retro_serialize(data: *mut c_void, size: usize) -> bool {
    if data.is_null() {
        return false;
    }
    if let Ok(guard) = CORE.lock() {
        if let Some(player) = guard.as_ref() {
            let blob = player.save_state();
            if blob.len() > size {
                return false;
            }
            unsafe {
                ptr::copy_nonoverlapping(blob.as_ptr(), data as *mut u8, blob.len());
            }
            return true;
        }
    }
    false
}

#[no_mangle]
pub extern "C" fn retro_unserialize(data: *const c_void, size: usize) -> bool {
    if data.is_null() {
        return false;
    }
    let mut buf = vec![0u8; size];
    unsafe {
        ptr::copy_nonoverlapping(data as *const u8, buf.as_mut_ptr(), size);
    }
    if let Ok(mut guard) = CORE.lock() {
        if let Some(player) = guard.as_mut() {
            return player.load_state(&buf).is_ok();
        }
    }
    false
}

#[no_mangle]
pub extern "C" fn retro_cheat_reset() {}

#[no_mangle]
pub extern "C" fn retro_cheat_set(_index: c_uint, _enabled: bool, _code: *const c_char) {}

#[no_mangle]
pub extern "C" fn retro_get_memory_data(_id: c_uint) -> *mut c_void {
    ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn retro_get_memory_size(_id: c_uint) -> usize {
    0
}

// ============================================================
// Helpers: pixel format, input, core options
// ============================================================

fn set_pixel_format() -> bool {
    let mut pixel_format = RETRO_PIXEL_FORMAT_XRGB8888;
    callbacks::environment(
        RETRO_ENVIRONMENT_SET_PIXEL_FORMAT,
        &mut pixel_format as *mut c_int as *mut c_void,
    )
}

fn set_performance_level() {
    let mut level = PERFORMANCE_LEVEL;
    callbacks::environment(
        RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL,
        (&mut level as *mut c_uint).cast(),
    );
}

fn input_descriptors() -> [RetroInputDescriptor; 9] {
    let descriptor = |id: c_uint, description: &'static CStr| RetroInputDescriptor {
        port: 0,
        device: RETRO_DEVICE_JOYPAD,
        index: 0,
        id,
        description: description.as_ptr(),
    };
    [
        descriptor(RETRO_DEVICE_ID_JOYPAD_UP, c"D-Pad Up"),
        descriptor(RETRO_DEVICE_ID_JOYPAD_DOWN, c"D-Pad Down"),
        descriptor(RETRO_DEVICE_ID_JOYPAD_LEFT, c"D-Pad Left"),
        descriptor(RETRO_DEVICE_ID_JOYPAD_RIGHT, c"D-Pad Right"),
        descriptor(RETRO_DEVICE_ID_JOYPAD_A, c"A"),
        descriptor(RETRO_DEVICE_ID_JOYPAD_B, c"B"),
        descriptor(RETRO_DEVICE_ID_JOYPAD_START, c"Start"),
        descriptor(RETRO_DEVICE_ID_JOYPAD_SELECT, c"Select"),
        RetroInputDescriptor {
            port: 0,
            device: 0,
            index: 0,
            id: 0,
            description: ptr::null(),
        },
    ]
}

fn register_input_descriptors() {
    let descriptors = input_descriptors();
    callbacks::environment(
        RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS,
        descriptors.as_ptr().cast_mut().cast(),
    );
}

fn poll_input() {
    callbacks::input_poll();
    let Ok(mut guard) = CORE.lock() else {
        return;
    };
    let Some(player) = guard.as_mut() else {
        return;
    };

    let a = callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_A) != 0;
    let b = callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_B) != 0;
    let swap = SWAP_AB.load(Ordering::Relaxed);
    let mut buttons = InputButtons::default();
    buttons.a = if swap { b } else { a };
    buttons.b = if swap { a } else { b };
    buttons.start =
        callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_START) != 0;
    buttons.select =
        callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_SELECT) != 0;
    buttons.up = callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_UP) != 0;
    buttons.down =
        callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_DOWN) != 0;
    buttons.left =
        callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_LEFT) != 0;
    buttons.right =
        callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_RIGHT) != 0;
    let _ = RETRO_DEVICE_ID_JOYPAD_X;
    let _ = RETRO_DEVICE_ID_JOYPAD_Y;
    player.set_input(buttons);
}

fn scale_volume(samples: &mut [i16], volume: u8) {
    if volume >= 100 {
        return;
    }
    let scale = f32::from(volume) / 100.0;
    for s in samples.iter_mut() {
        *s = (*s as f32 * scale) as i16;
    }
}

fn core_option_variables() -> [RetroVariable; 4] {
    [
        RetroVariable {
            key: c"playdiaemu_volume".as_ptr(),
            value: c"Audio Volume (%); 100|90|80|70|60|50|40|30|20|10|0".as_ptr(),
        },
        RetroVariable {
            key: c"playdiaemu_swap_ab".as_ptr(),
            value: c"Swap A/B Buttons; disabled|enabled".as_ptr(),
        },
        RetroVariable {
            key: c"playdiaemu_debug_logging".as_ptr(),
            value: c"Debug Logging; disabled|enabled".as_ptr(),
        },
        RetroVariable {
            key: ptr::null(),
            value: ptr::null(),
        },
    ]
}

fn set_core_options() {
    let variables = core_option_variables();
    callbacks::environment(
        RETRO_ENVIRONMENT_SET_VARIABLES,
        variables.as_ptr() as *mut c_void,
    );
}

fn get_core_option(key: &CStr) -> Option<String> {
    let mut variable = RetroVariable {
        key: key.as_ptr(),
        value: ptr::null(),
    };
    let success = callbacks::environment(
        RETRO_ENVIRONMENT_GET_VARIABLE,
        &mut variable as *mut _ as *mut c_void,
    );
    if success && !variable.value.is_null() {
        unsafe {
            CStr::from_ptr(variable.value)
                .to_str()
                .ok()
                .map(str::to_owned)
        }
    } else {
        None
    }
}

fn core_options_changed() -> bool {
    let mut updated = false;
    let success = callbacks::environment(
        RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE,
        (&mut updated as *mut bool).cast(),
    );
    success && updated
}

fn read_core_options(mut get: impl FnMut(&CStr) -> Option<String>) -> CoreOptions {
    let mut config = CoreOptions::default();
    if let Some(value) = get(c"playdiaemu_volume").and_then(|value| value.parse().ok()) {
        config.volume = value;
    }
    if let Some(swap) = get(c"playdiaemu_swap_ab") {
        config.swap_ab = swap == "enabled";
    }
    if let Some(debug) = get(c"playdiaemu_debug_logging") {
        config.debug_logging = debug == "enabled";
    }
    config
}

fn apply_core_options() {
    let config = read_core_options(get_core_option);
    logger::set_debug_logging(config.debug_logging);
    VOLUME.store(config.volume, Ordering::Relaxed);
    SWAP_AB.store(config.swap_ab, Ordering::Relaxed);
    log::info!(
        "Core options applied: volume={} swap_ab={} debug_logging={}",
        config.volume,
        config.swap_ab,
        config.debug_logging
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logger::debug_logging_enabled;
    use log::LevelFilter;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    static ACCEPT_FORMAT: AtomicBool = AtomicBool::new(false);
    static VIDEO_SEEN: AtomicBool = AtomicBool::new(false);
    static VIDEO_DIMS_OK: AtomicBool = AtomicBool::new(false);
    static INPUT_DESCRIPTORS_SET: AtomicBool = AtomicBool::new(false);
    static PERFORMANCE_LEVEL_SET: AtomicBool = AtomicBool::new(false);
    static LOG_INTERFACE_SET: AtomicBool = AtomicBool::new(false);
    static CORE_OPTIONS_SET: AtomicBool = AtomicBool::new(false);
    static CORE_OPTIONS_UPDATED: AtomicBool = AtomicBool::new(false);
    static TEST_VOLUME: Mutex<u8> = Mutex::new(100);
    static TEST_SWAP_AB: Mutex<&'static str> = Mutex::new("disabled");
    static TEST_DEBUG: Mutex<&'static str> = Mutex::new("disabled");
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    unsafe extern "C" fn test_log(_level: c_uint, _fmt: *const c_char) {}

    unsafe extern "C" fn environment(cmd: c_uint, data: *mut c_void) -> bool {
        match cmd {
            RETRO_ENVIRONMENT_SET_PIXEL_FORMAT => {
                let format = *(data as *const c_int);
                format == 1 && ACCEPT_FORMAT.load(Ordering::SeqCst)
            }
            RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS => {
                let first = *(data as *const RetroInputDescriptor);
                INPUT_DESCRIPTORS_SET.store(
                    !first.description.is_null() && first.device == RETRO_DEVICE_JOYPAD,
                    Ordering::SeqCst,
                );
                true
            }
            RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL => {
                let level = *(data as *const c_uint);
                PERFORMANCE_LEVEL_SET.store(level == PERFORMANCE_LEVEL, Ordering::SeqCst);
                true
            }
            RETRO_ENVIRONMENT_GET_LOG_INTERFACE => {
                let cb = data as *mut RetroLogCallback;
                (*cb).log = Some(test_log);
                LOG_INTERFACE_SET.store(true, Ordering::SeqCst);
                true
            }
            RETRO_ENVIRONMENT_SET_VARIABLES => {
                let mut cur = data as *const RetroVariable;
                let mut found_volume = false;
                loop {
                    let var = *cur;
                    if var.key.is_null() {
                        break;
                    }
                    let key = CStr::from_ptr(var.key).to_string_lossy();
                    if key == "playdiaemu_volume" {
                        found_volume = true;
                    }
                    cur = cur.wrapping_add(1);
                }
                CORE_OPTIONS_SET.store(found_volume, Ordering::SeqCst);
                true
            }
            RETRO_ENVIRONMENT_GET_VARIABLE => {
                let var = data as *mut RetroVariable;
                let key = CStr::from_ptr((*var).key).to_string_lossy();
                let value: *const c_char = match key.as_ref() {
                    "playdiaemu_volume" => {
                        let v = *TEST_VOLUME.lock().unwrap();
                        match v {
                            50 => c"50".as_ptr(),
                            _ => c"100".as_ptr(),
                        }
                    }
                    "playdiaemu_swap_ab" => match *TEST_SWAP_AB.lock().unwrap() {
                        "enabled" => c"enabled".as_ptr(),
                        _ => c"disabled".as_ptr(),
                    },
                    "playdiaemu_debug_logging" => match *TEST_DEBUG.lock().unwrap() {
                        "enabled" => c"enabled".as_ptr(),
                        _ => c"disabled".as_ptr(),
                    },
                    _ => return false,
                };
                (*var).value = value;
                true
            }
            RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE => {
                let flag = data as *mut bool;
                let updated = CORE_OPTIONS_UPDATED.swap(false, Ordering::SeqCst);
                *flag = updated;
                true
            }
            _ => true,
        }
    }

    unsafe extern "C" fn video(data: *const c_void, width: c_uint, height: c_uint, pitch: usize) {
        VIDEO_SEEN.store(!data.is_null(), Ordering::SeqCst);
        VIDEO_DIMS_OK.store(
            width == FB_WIDTH as c_uint && height == FB_HEIGHT as c_uint && pitch == FB_WIDTH * 4,
            Ordering::SeqCst,
        );
    }

    fn synthetic_raw_disc() -> Vec<u8> {
        let raw_sectors = 4;
        let mut raw = vec![0u8; 2352 * raw_sectors];
        for i in 0..raw_sectors {
            let off = i * 2352;
            raw[off + 15] = 2;
            raw[off + 16] = 0;
            raw[off + 17] = 0;
            raw[off + 18] = 0x08;
            raw[off + 22] = 0x08;
            raw[off + 24] = 0xF1;
            for b in 1..64 {
                raw[off + 24 + b] = b as u8;
            }
        }
        raw
    }

    fn game_info_for_bytes(bytes: &[u8]) -> RetroGameInfo {
        RetroGameInfo {
            path: ptr::null(),
            data: bytes.as_ptr() as *const c_void,
            size: bytes.len(),
            meta: ptr::null(),
        }
    }

    #[test]
    fn negotiates_xrgb8888_and_runs_disc_player() {
        let _guard = TEST_LOCK.lock().unwrap();

        *TEST_VOLUME.lock().unwrap() = 100;
        *TEST_SWAP_AB.lock().unwrap() = "disabled";
        *TEST_DEBUG.lock().unwrap() = "disabled";
        VOLUME.store(100, Ordering::SeqCst);
        SWAP_AB.store(false, Ordering::SeqCst);
        logger::set_debug_logging(false);
        CORE_OPTIONS_SET.store(false, Ordering::SeqCst);

        retro_set_environment(Some(environment));
        retro_set_video_refresh(Some(video));
        assert!(
            CORE_OPTIONS_SET.load(Ordering::SeqCst),
            "SET_VARIABLES should include volume"
        );

        ACCEPT_FORMAT.store(true, Ordering::SeqCst);
        VIDEO_SEEN.store(false, Ordering::SeqCst);
        VIDEO_DIMS_OK.store(false, Ordering::SeqCst);
        INPUT_DESCRIPTORS_SET.store(false, Ordering::SeqCst);
        PERFORMANCE_LEVEL_SET.store(false, Ordering::SeqCst);
        LOG_INTERFACE_SET.store(false, Ordering::SeqCst);

        retro_init();
        assert!(LOG_INTERFACE_SET.load(Ordering::SeqCst));

        assert!(!retro_load_game(ptr::null()));
        assert!(CORE.lock().unwrap().is_none());

        let disc = synthetic_raw_disc();
        let info = game_info_for_bytes(&disc);
        assert!(retro_load_game(&info));
        assert!(CORE.lock().unwrap().is_some());
        assert!(INPUT_DESCRIPTORS_SET.load(Ordering::SeqCst));
        assert!(PERFORMANCE_LEVEL_SET.load(Ordering::SeqCst));
        assert_eq!(VOLUME.load(Ordering::SeqCst), 100);
        assert!(!SWAP_AB.load(Ordering::SeqCst));

        *TEST_VOLUME.lock().unwrap() = 50;
        *TEST_SWAP_AB.lock().unwrap() = "enabled";
        *TEST_DEBUG.lock().unwrap() = "enabled";
        CORE_OPTIONS_UPDATED.store(true, Ordering::SeqCst);
        retro_run();
        assert_eq!(VOLUME.load(Ordering::SeqCst), 50);
        assert!(SWAP_AB.load(Ordering::SeqCst));
        assert!(debug_logging_enabled());
        assert_eq!(log::max_level(), LevelFilter::Debug);

        *TEST_VOLUME.lock().unwrap() = 100;
        *TEST_SWAP_AB.lock().unwrap() = "disabled";
        *TEST_DEBUG.lock().unwrap() = "disabled";
        logger::set_debug_logging(false);
        VOLUME.store(100, Ordering::SeqCst);
        SWAP_AB.store(false, Ordering::SeqCst);

        assert!(VIDEO_SEEN.load(Ordering::SeqCst));
        assert!(VIDEO_DIMS_OK.load(Ordering::SeqCst));

        let n = retro_serialize_size();
        assert!(n > 0);
        let mut blob = vec![0u8; n];
        assert!(retro_serialize(blob.as_mut_ptr().cast(), blob.len()));
        retro_run();
        assert!(retro_unserialize(blob.as_ptr().cast(), blob.len()));
        assert_eq!(retro_serialize_size(), n);

        retro_reset();
        retro_unload_game();
        assert!(CORE.lock().unwrap().is_none());
        retro_deinit();
    }

    #[test]
    fn core_option_helpers_parse_defaults_and_volume_scale() {
        let defaults = read_core_options(|_| None);
        assert_eq!(defaults, CoreOptions::default());

        let configured = read_core_options(|key| match key.to_str().unwrap() {
            "playdiaemu_volume" => Some("50".into()),
            "playdiaemu_swap_ab" => Some("enabled".into()),
            "playdiaemu_debug_logging" => Some("enabled".into()),
            _ => None,
        });
        assert_eq!(configured.volume, 50);
        assert!(configured.swap_ab);
        assert!(configured.debug_logging);

        let mut samples = [100i16, -100, 0];
        scale_volume(&mut samples, 100);
        assert_eq!(samples, [100, -100, 0]);
        scale_volume(&mut samples, 50);
        assert_eq!(samples, [50, -50, 0]);
        scale_volume(&mut samples, 0);
        assert_eq!(samples, [0, 0, 0]);
    }
}
