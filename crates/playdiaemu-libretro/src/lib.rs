//! Minimal libretro core for Playdia.
//!
//! Implements the classic libretro C ABI so the core can be loaded by
//! RetroArch-compatible frontends once a disc image is supplied.
//! Uses the same HLE DiscPlayer path as the standalone emulator.

#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::chunks_exact_to_as_chunks)]
#![allow(clippy::if_same_then_else)]

use playdia_core::player::DiscPlayer;
use playdia_core::{InputButtons, FB_HEIGHT, FB_WIDTH};
use std::ffi::c_void;
use std::os::raw::{c_char, c_int, c_uint};
use std::ptr;
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
const RETRO_ENVIRONMENT_SET_PIXEL_FORMAT: c_uint = 10;
const RETRO_PIXEL_FORMAT_XRGB8888: c_int = 1;
const RETRO_REGION_NTSC: c_uint = 0;

/// Standalone HLE paces ~30 host frames/sec for disc video.
const HOST_FPS: f64 = 30.0;

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
}

static mut CALLBACKS: Callbacks = Callbacks {
    environment: None,
    video_refresh: None,
    audio_sample: None,
    audio_sample_batch: None,
    input_poll: None,
    input_state: None,
};

static CORE: Mutex<Option<DiscPlayer>> = Mutex::new(None);

fn cstr(bytes: &[u8]) -> *const c_char {
    bytes.as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn retro_api_version() -> c_int {
    RETRO_API_VERSION
}

#[no_mangle]
pub extern "C" fn retro_set_environment(cb: EnvironmentFn) {
    unsafe { CALLBACKS.environment = Some(cb) }
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
pub extern "C" fn retro_init() {}

#[no_mangle]
pub extern "C" fn retro_deinit() {
    *CORE.lock().unwrap() = None;
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
        let audio = p.drain_audio();
        if !audio.is_empty() {
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
    let mut b = InputButtons::default();
    b.b = state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_B) != 0;
    b.a = state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_A) != 0;
    b.start = state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_START) != 0;
    b.select = state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_SELECT) != 0;
    b.up = state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_UP) != 0;
    b.down = state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_DOWN) != 0;
    b.left = state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_LEFT) != 0;
    b.right = state(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_RIGHT) != 0;
    let _ = RETRO_DEVICE_ID_JOYPAD_X;
    let _ = RETRO_DEVICE_ID_JOYPAD_Y;
    p.set_input(b);
}

#[no_mangle]
pub extern "C" fn retro_serialize_size() -> usize {
    // HLE DiscPlayer has no machine save-state blob (matches standalone).
    0
}

#[no_mangle]
pub extern "C" fn retro_serialize(_data: *mut c_void, _size: usize) -> bool {
    false
}

#[no_mangle]
pub extern "C" fn retro_unserialize(_data: *const c_void, _size: usize) -> bool {
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
    *CORE.lock().unwrap() = Some(p);
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

    extern "C" fn environment(cmd: c_uint, data: *mut c_void) -> bool {
        if cmd == RETRO_ENVIRONMENT_SET_PIXEL_FORMAT {
            let format = unsafe { *(data as *const c_int) };
            return format == 1 && ACCEPT_FORMAT.load(Ordering::SeqCst);
        }
        true
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
        retro_set_environment(environment);
        retro_set_video_refresh(video);
        ACCEPT_FORMAT.store(true, Ordering::SeqCst);
        VIDEO_SEEN.store(false, Ordering::SeqCst);
        VIDEO_DIMS_OK.store(false, Ordering::SeqCst);

        assert!(!retro_load_game(ptr::null()));
        assert!(CORE.lock().unwrap().is_none());

        let disc = synthetic_raw_disc();
        let info = game_info_for_bytes(&disc);
        assert!(retro_load_game(&info));
        assert!(CORE.lock().unwrap().is_some());
        assert_eq!(retro_serialize_size(), 0);

        retro_run();
        assert!(VIDEO_SEEN.load(Ordering::SeqCst));
        assert!(VIDEO_DIMS_OK.load(Ordering::SeqCst));

        retro_reset();
        retro_unload_game();
        assert!(CORE.lock().unwrap().is_none());
    }
}
