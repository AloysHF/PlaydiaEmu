//! Minimal libretro core for Playdia.
//!
//! Implements the classic libretro C ABI so the core can be loaded by
//! RetroArch-compatible frontends once a disc image is supplied.
//! Uses the same HLE DiscPlayer path as the standalone emulator.

#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::chunks_exact_to_as_chunks)]
#![allow(clippy::if_same_then_else)]

use log::{Level, LevelFilter, Log, Metadata, Record};
use playdia_core::player::DiscPlayer;
use playdia_core::{InputButtons, FB_HEIGHT, FB_WIDTH};
use std::ffi::{c_void, CStr, CString};
use std::os::raw::{c_char, c_int, c_uint};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;

const RETRO_API_VERSION: c_int = 1;
const RETRO_DEVICE_JOYPAD: c_uint = 1;
const RETRO_DEVICE_ID_JOYPAD_B: c_uint = 0;
const RETRO_DEVICE_ID_JOYPAD_Y: c_uint = 1;
const RETRO_DEVICE_ID_JOYPAD_SELECT: c_uint = 2;
const RETRO_DEVICE_ID_JOYPAD_START: c_uint = 3;
const RETRO_DEVICE_ID_JOYPAD_UP: c_uint = 4;
const RETRO_DEVICE_ID_JOYPAD_DOWN: c_uint = 5;
const RETRO_DEVICE_ID_JOYPAD_LEFT: c_uint = 6;
const RETRO_DEVICE_ID_JOYPAD_RIGHT: c_uint = 7;
const RETRO_DEVICE_ID_JOYPAD_A: c_uint = 8;
const RETRO_DEVICE_ID_JOYPAD_X: c_uint = 9;
const RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL: c_uint = 8;
const RETRO_ENVIRONMENT_SET_PIXEL_FORMAT: c_uint = 10;
const RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS: c_uint = 11;
const RETRO_ENVIRONMENT_GET_VARIABLE: c_uint = 15;
const RETRO_ENVIRONMENT_SET_VARIABLES: c_uint = 16;
const RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE: c_uint = 17;
const RETRO_ENVIRONMENT_GET_LOG_INTERFACE: c_uint = 27;
const RETRO_PIXEL_FORMAT_XRGB8888: c_int = 1;
const RETRO_REGION_NTSC: c_uint = 0;
const RETRO_LOG_DEBUG: c_int = 0;
const RETRO_LOG_INFO: c_int = 1;
const RETRO_LOG_WARN: c_int = 2;
const RETRO_LOG_ERROR: c_int = 3;

/// Standalone HLE paces ~30 host frames/sec for disc video.
const HOST_FPS: f64 = 30.0;
/// Match SPMP8000/Dingoo HLE shells.
const PERFORMANCE_LEVEL: c_uint = 4;

type LogFn = unsafe extern "C" fn(level: c_int, fmt: *const c_char);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GameInfo {
    path: *const c_char,
    data: *const c_void,
    size: usize,
    meta: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SystemInfo {
    library_name: *const c_char,
    library_version: *const c_char,
    valid_extensions: *const c_char,
    need_fullpath: u8,
    block_extract: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GameGeometry {
    base_width: c_uint,
    base_height: c_uint,
    max_width: c_uint,
    max_height: c_uint,
    aspect_ratio: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SystemTiming {
    fps: f64,
    sample_rate: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RetroSystemAvInfo {
    geometry: GameGeometry,
    timing: SystemTiming,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RetroInputDescriptor {
    port: c_uint,
    device: c_uint,
    index: c_uint,
    id: c_uint,
    description: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RetroLogCallback {
    log: LogFn,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RetroVariable {
    key: *const c_char,
    value: *const c_char,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CoreOptions {
    volume: u8,
    swap_ab: bool,
    debug_logging: bool,
}

impl Default for CoreOptions {
    fn default() -> Self {
        Self {
            volume: 100,
            swap_ab: false,
            debug_logging: false,
        }
    }
}

type EnvironmentFn = extern "C" fn(cmd: c_uint, data: *mut c_void) -> bool;
type VideoRefreshFn =
    extern "C" fn(data: *const c_void, width: c_uint, height: c_uint, pitch: usize);
type AudioSampleFn = extern "C" fn(left: i16, right: i16);
type AudioSampleBatchFn = extern "C" fn(data: *const i16, frames: usize) -> usize;
type InputPollFn = extern "C" fn();
type InputStateFn = extern "C" fn(port: c_uint, device: c_uint, index: c_uint, id: c_uint) -> i16;

struct Callbacks {
    environment: Option<EnvironmentFn>,
    video_refresh: Option<VideoRefreshFn>,
    audio_sample: Option<AudioSampleFn>,
    audio_sample_batch: Option<AudioSampleBatchFn>,
    input_poll: Option<InputPollFn>,
    input_state: Option<InputStateFn>,
    log: Option<LogFn>,
}

static mut CALLBACKS: Callbacks = Callbacks {
    environment: None,
    video_refresh: None,
    audio_sample: None,
    audio_sample_batch: None,
    input_poll: None,
    input_state: None,
    log: None,
};

static CORE: Mutex<Option<DiscPlayer>> = Mutex::new(None);
static LOGGER: LibretroLogger = LibretroLogger;
static DEBUG_LOGGING: AtomicBool = AtomicBool::new(false);
static VOLUME: AtomicU8 = AtomicU8::new(100);
static SWAP_AB: AtomicBool = AtomicBool::new(false);

struct LibretroLogger;

impl Log for LibretroLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info || DEBUG_LOGGING.load(Ordering::Relaxed)
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let level = match record.level() {
            Level::Error => RETRO_LOG_ERROR,
            Level::Warn => RETRO_LOG_WARN,
            Level::Info => RETRO_LOG_INFO,
            Level::Debug | Level::Trace => RETRO_LOG_DEBUG,
        };
        // Frontend log is printf-style; escape `%` and pass the full line as fmt.
        let text = format!("[PlaydiaEmu] {}\n", record.args()).replace('%', "%%");
        let Ok(message) = CString::new(text) else {
            return;
        };
        if let Some(log_fn) = unsafe { CALLBACKS.log } {
            unsafe { log_fn(level, message.as_ptr()) };
        }
    }

    fn flush(&self) {}
}

fn cstr(bytes: &[u8]) -> *const c_char {
    bytes.as_ptr() as *const c_char
}

fn environment(cmd: c_uint, data: *mut c_void) -> bool {
    unsafe {
        match CALLBACKS.environment {
            Some(env) => env(cmd, data),
            None => false,
        }
    }
}

fn init_log() {
    unsafe extern "C" fn unused_log(_level: c_int, _fmt: *const c_char) {}
    let mut log_cb = RetroLogCallback { log: unused_log };
    if environment(
        RETRO_ENVIRONMENT_GET_LOG_INTERFACE,
        &mut log_cb as *mut _ as *mut c_void,
    ) {
        unsafe { CALLBACKS.log = Some(log_cb.log) };
    } else {
        unsafe { CALLBACKS.log = Some(unused_log) };
    }
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(LevelFilter::Info);
}

fn set_performance_level() {
    let mut level = PERFORMANCE_LEVEL;
    environment(
        RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL,
        &mut level as *mut c_uint as *mut c_void,
    );
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
    environment(
        RETRO_ENVIRONMENT_SET_VARIABLES,
        variables.as_ptr() as *mut c_void,
    );
}

fn get_core_option(key: &CStr) -> Option<String> {
    let mut variable = RetroVariable {
        key: key.as_ptr(),
        value: ptr::null(),
    };
    let success = environment(
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
    let success = environment(
        RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE,
        &mut updated as *mut _ as *mut c_void,
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
    set_debug_logging(config.debug_logging);
    VOLUME.store(config.volume, Ordering::Relaxed);
    SWAP_AB.store(config.swap_ab, Ordering::Relaxed);
    log::info!(
        "Core options applied: volume={} swap_ab={} debug_logging={}",
        config.volume,
        config.swap_ab,
        config.debug_logging
    );
}

fn set_debug_logging(enabled: bool) {
    DEBUG_LOGGING.store(enabled, Ordering::Relaxed);
    log::set_max_level(if enabled {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    });
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
    environment(
        RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS,
        descriptors.as_ptr() as *mut c_void,
    );
}

#[no_mangle]
pub extern "C" fn retro_api_version() -> c_int {
    RETRO_API_VERSION
}

#[no_mangle]
pub extern "C" fn retro_set_environment(cb: EnvironmentFn) {
    unsafe { CALLBACKS.environment = Some(cb) }
    set_core_options();
}

#[no_mangle]
pub extern "C" fn retro_set_video_refresh(cb: VideoRefreshFn) {
    unsafe { CALLBACKS.video_refresh = Some(cb) }
}

#[no_mangle]
pub extern "C" fn retro_set_audio_sample(cb: AudioSampleFn) {
    unsafe { CALLBACKS.audio_sample = Some(cb) }
}

#[no_mangle]
pub extern "C" fn retro_set_audio_sample_batch(cb: AudioSampleBatchFn) {
    unsafe { CALLBACKS.audio_sample_batch = Some(cb) }
}

#[no_mangle]
pub extern "C" fn retro_set_input_poll(cb: InputPollFn) {
    unsafe { CALLBACKS.input_poll = Some(cb) }
}

#[no_mangle]
pub extern "C" fn retro_set_input_state(cb: InputStateFn) {
    unsafe { CALLBACKS.input_state = Some(cb) }
}

#[no_mangle]
pub extern "C" fn retro_get_system_info(info: *mut SystemInfo) {
    if info.is_null() {
        return;
    }
    unsafe {
        *info = SystemInfo {
            library_name: cstr(b"PlaydiaEmu\0"),
            library_version: cstr(b"0.1.0\0"),
            valid_extensions: cstr(b"cue|iso|bin|zip\0"),
            need_fullpath: 1,
            // Pass ZIP paths to the core; do not let the frontend extract them.
            block_extract: 1,
        };
    }
}

#[no_mangle]
pub extern "C" fn retro_get_system_av_info(info: *mut RetroSystemAvInfo) {
    if info.is_null() {
        return;
    }
    unsafe {
        *info = RetroSystemAvInfo {
            geometry: GameGeometry {
                base_width: FB_WIDTH as c_uint,
                base_height: FB_HEIGHT as c_uint,
                max_width: FB_WIDTH as c_uint,
                max_height: FB_HEIGHT as c_uint,
                aspect_ratio: 4.0 / 3.0,
            },
            timing: SystemTiming {
                fps: HOST_FPS,
                sample_rate: 44100.0,
            },
        };
    }
}

#[no_mangle]
pub extern "C" fn retro_init() {
    init_log();
    log::info!("PlaydiaEmu libretro core initialized");
}

#[no_mangle]
pub extern "C" fn retro_deinit() {
    *CORE.lock().unwrap() = None;
    log::info!("PlaydiaEmu libretro core deinitialized");
}

#[no_mangle]
pub extern "C" fn retro_set_controller_port_device(_port: c_uint, _device: c_uint) {}

#[no_mangle]
pub extern "C" fn retro_reset() {
    if let Ok(mut guard) = CORE.lock() {
        if let Some(m) = guard.as_mut() {
            m.reset();
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_run() {
    if core_options_changed() {
        apply_core_options();
    }
    poll_input();
    if let Ok(mut guard) = CORE.lock() {
        let Some(p) = guard.as_mut() else { return };
        let _ = p.run_frame();
        let fb = p.framebuffer();
        if let Some(cb) = unsafe { CALLBACKS.video_refresh } {
            cb(
                fb.as_ptr() as *const c_void,
                FB_WIDTH as c_uint,
                FB_HEIGHT as c_uint,
                FB_WIDTH * 4,
            );
        }
        let mut audio = p.drain_audio();
        if !audio.is_empty() {
            scale_volume(&mut audio, VOLUME.load(Ordering::Relaxed));
            if let Some(batch) = unsafe { CALLBACKS.audio_sample_batch } {
                batch(audio.as_ptr(), audio.len() / 2);
            } else if let Some(sample) = unsafe { CALLBACKS.audio_sample } {
                for pair in audio.chunks_exact(2) {
                    sample(pair[0], pair[1]);
                }
            }
        }
    }
}

fn poll_input() {
    let Ok(mut guard) = CORE.lock() else { return };
    let Some(p) = guard.as_mut() else { return };
    if let Some(poll) = unsafe { CALLBACKS.input_poll } {
        poll();
    }
    let Some(state) = (unsafe { CALLBACKS.input_state }) else {
        return;
    };
    let a = state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_A) != 0;
    let b = state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_B) != 0;
    let swap = SWAP_AB.load(Ordering::Relaxed);
    let mut buttons = InputButtons::default();
    buttons.a = if swap { b } else { a };
    buttons.b = if swap { a } else { b };
    buttons.start = state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_START) != 0;
    buttons.select = state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_SELECT) != 0;
    buttons.up = state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_UP) != 0;
    buttons.down = state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_DOWN) != 0;
    buttons.left = state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_LEFT) != 0;
    buttons.right = state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_RIGHT) != 0;
    let _ = RETRO_DEVICE_ID_JOYPAD_X;
    let _ = RETRO_DEVICE_ID_JOYPAD_Y;
    p.set_input(buttons);
}

#[no_mangle]
pub extern "C" fn retro_serialize_size() -> usize {
    if let Ok(guard) = CORE.lock() {
        if let Some(p) = guard.as_ref() {
            return p.save_state().len();
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
        if let Some(p) = guard.as_ref() {
            let blob = p.save_state();
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
        if let Some(p) = guard.as_mut() {
            return p.load_state(&buf).is_ok();
        }
    }
    false
}

#[no_mangle]
pub extern "C" fn retro_cheat_reset() {}

#[no_mangle]
pub extern "C" fn retro_cheat_set(_index: c_uint, _enabled: bool, _code: *const c_char) {}

#[no_mangle]
pub extern "C" fn retro_load_game(game: *const GameInfo) -> bool {
    if game.is_null() {
        return false;
    }
    let g = unsafe { &*game };
    let mut p = DiscPlayer::new();
    if !g.path.is_null() {
        let path = unsafe {
            let mut len = 0usize;
            while *g.path.add(len) != 0 {
                len += 1;
            }
            std::slice::from_raw_parts(g.path as *const u8, len)
        };
        let Ok(s) = std::str::from_utf8(path) else {
            return false;
        };
        if p.load_path(std::path::Path::new(s)).is_err() {
            return false;
        }
    } else if !g.data.is_null() && g.size > 0 {
        let bytes = unsafe { std::slice::from_raw_parts(g.data as *const u8, g.size).to_vec() };
        if p.load_bytes(bytes).is_err() {
            return false;
        }
    } else {
        return false;
    }
    let Some(env) = (unsafe { CALLBACKS.environment }) else {
        return false;
    };
    let mut fmt = RETRO_PIXEL_FORMAT_XRGB8888;
    if !env(
        RETRO_ENVIRONMENT_SET_PIXEL_FORMAT,
        &mut fmt as *mut c_int as *mut c_void,
    ) {
        return false;
    }
    register_input_descriptors();
    set_performance_level();
    apply_core_options();
    *CORE.lock().unwrap() = Some(p);
    log::info!("Loaded HLE disc player content");
    true
}

#[no_mangle]
pub extern "C" fn retro_load_game_special(
    _game_type: c_uint,
    _info: *const GameInfo,
    _num_info: usize,
) -> bool {
    false
}

#[no_mangle]
pub extern "C" fn retro_unload_game() {
    *CORE.lock().unwrap() = None;
}

#[no_mangle]
pub extern "C" fn retro_get_region() -> c_uint {
    RETRO_REGION_NTSC
}

#[no_mangle]
pub extern "C" fn retro_get_memory_data(id: c_uint) -> *mut c_void {
    let _ = id;
    ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn retro_get_memory_size(id: c_uint) -> usize {
    let _ = id;
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    static ACCEPT_FORMAT: AtomicBool = AtomicBool::new(false);
    static VIDEO_SEEN: AtomicBool = AtomicBool::new(false);
    static VIDEO_DIMS_OK: AtomicBool = AtomicBool::new(false);
    static INPUT_DESCRIPTORS_SET: AtomicBool = AtomicBool::new(false);
    static PERFORMANCE_LEVEL_SET: AtomicBool = AtomicBool::new(false);
    static LOG_INTERFACE_SET: AtomicBool = AtomicBool::new(false);
    static CORE_OPTIONS_SET: AtomicBool = AtomicBool::new(false);
    static CORE_OPTIONS_UPDATED: AtomicBool = AtomicBool::new(false);
    static TEST_VOLUME: std::sync::Mutex<u8> = std::sync::Mutex::new(100);
    static TEST_SWAP_AB: std::sync::Mutex<&'static str> = std::sync::Mutex::new("disabled");
    static TEST_DEBUG: std::sync::Mutex<&'static str> = std::sync::Mutex::new("disabled");

    unsafe extern "C" fn test_log(_level: c_int, _fmt: *const c_char) {}

    extern "C" fn environment(cmd: c_uint, data: *mut c_void) -> bool {
        match cmd {
            RETRO_ENVIRONMENT_SET_PIXEL_FORMAT => {
                let format = unsafe { *(data as *const c_int) };
                format == 1 && ACCEPT_FORMAT.load(Ordering::SeqCst)
            }
            RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS => {
                let first = unsafe { *(data as *const RetroInputDescriptor) };
                INPUT_DESCRIPTORS_SET.store(
                    !first.description.is_null() && first.device == RETRO_DEVICE_JOYPAD,
                    Ordering::SeqCst,
                );
                true
            }
            RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL => {
                let level = unsafe { *(data as *const c_uint) };
                PERFORMANCE_LEVEL_SET.store(level == PERFORMANCE_LEVEL, Ordering::SeqCst);
                true
            }
            RETRO_ENVIRONMENT_GET_LOG_INTERFACE => {
                let cb = data as *mut RetroLogCallback;
                unsafe { (*cb).log = test_log };
                LOG_INTERFACE_SET.store(true, Ordering::SeqCst);
                true
            }
            RETRO_ENVIRONMENT_SET_VARIABLES => {
                let mut cur = data as *const RetroVariable;
                let mut found_volume = false;
                loop {
                    let var = unsafe { *cur };
                    if var.key.is_null() {
                        break;
                    }
                    let key = unsafe { CStr::from_ptr(var.key) }.to_string_lossy();
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
                let key = unsafe { CStr::from_ptr((*var).key) }.to_string_lossy();
                let value: *const c_char = match key.as_ref() {
                    "playdiaemu_volume" => {
                        let v = *TEST_VOLUME.lock().unwrap();
                        match v {
                            100 => c"100".as_ptr(),
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
                unsafe { (*var).value = value };
                true
            }
            RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE => {
                let flag = data as *mut bool;
                let updated = CORE_OPTIONS_UPDATED.swap(false, Ordering::SeqCst);
                unsafe { *flag = updated };
                true
            }
            _ => true,
        }
    }

    extern "C" fn video(data: *const c_void, width: c_uint, height: c_uint, pitch: usize) {
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

    fn game_info_for_bytes(bytes: &[u8]) -> GameInfo {
        GameInfo {
            path: ptr::null(),
            data: bytes.as_ptr() as *const c_void,
            size: bytes.len(),
            meta: ptr::null(),
        }
    }

    #[test]
    fn negotiates_xrgb8888_and_runs_disc_player() {
        // Share statics with any concurrent libretro tests in this process.
        static TEST_LOCK: Mutex<()> = Mutex::new(());
        let _guard = TEST_LOCK.lock().unwrap();

        *TEST_VOLUME.lock().unwrap() = 100;
        *TEST_SWAP_AB.lock().unwrap() = "disabled";
        *TEST_DEBUG.lock().unwrap() = "disabled";
        VOLUME.store(100, Ordering::SeqCst);
        SWAP_AB.store(false, Ordering::SeqCst);
        set_debug_logging(false);
        CORE_OPTIONS_SET.store(false, Ordering::SeqCst);

        // Rebind environment after acquiring the shared test lock.
        retro_set_environment(environment);
        retro_set_video_refresh(video);
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

        // Runtime option change: volume 50, swap AB, debug on.
        *TEST_VOLUME.lock().unwrap() = 50;
        *TEST_SWAP_AB.lock().unwrap() = "enabled";
        *TEST_DEBUG.lock().unwrap() = "enabled";
        CORE_OPTIONS_UPDATED.store(true, Ordering::SeqCst);
        retro_run();
        assert_eq!(VOLUME.load(Ordering::SeqCst), 50);
        assert!(SWAP_AB.load(Ordering::SeqCst));
        assert!(DEBUG_LOGGING.load(Ordering::SeqCst));
        assert_eq!(log::max_level(), LevelFilter::Debug);

        // Reset for other tests in the process.
        *TEST_VOLUME.lock().unwrap() = 100;
        *TEST_SWAP_AB.lock().unwrap() = "disabled";
        *TEST_DEBUG.lock().unwrap() = "disabled";
        set_debug_logging(false);
        VOLUME.store(100, Ordering::SeqCst);
        SWAP_AB.store(false, Ordering::SeqCst);

        assert!(VIDEO_SEEN.load(Ordering::SeqCst));
        assert!(VIDEO_DIMS_OK.load(Ordering::SeqCst));

        // Save-state roundtrip on the same disc.
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
        assert_eq!(defaults.volume, 100);
        assert!(!defaults.swap_ab);
        assert!(!defaults.debug_logging);

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
