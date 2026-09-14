//! Minimal libretro core for Playdia.
//!
//! Implements the classic libretro C ABI so the core can be loaded by
//! RetroArch-compatible frontends once a disc image is supplied.

#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::chunks_exact_to_as_chunks)]
#![allow(clippy::if_same_then_else)]

use playdia_core::machine::{Machine, MachineConfig};
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
const RETRO_MEMORY_SAVE_RAM: c_uint = 0;
const RETRO_ENVIRONMENT_SET_PIXEL_FORMAT: c_uint = 10;
const RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME: c_uint = 18;
const RETRO_PIXEL_FORMAT_XRGB8888: c_int = 1;
const RETRO_REGION_NTSC: c_uint = 0;

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
    aspect_num: c_uint,
    aspect_den: c_uint,
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

static CORE: Mutex<Option<Machine>> = Mutex::new(None);

fn cstr(bytes: &[u8]) -> *const c_char {
    bytes.as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn retro_api_version() -> c_int {
    RETRO_API_VERSION
}

#[no_mangle]
pub extern "C" fn retro_set_environment(cb: EnvironmentFn) {
    unsafe {
        CALLBACKS.environment = Some(cb);
        let mut no_game: u8 = 1;
        cb(
            RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME,
            &mut no_game as *mut u8 as *mut c_void,
        );
    }
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
            valid_extensions: cstr(b"cue|iso|bin\0"),
            need_fullpath: 1,
            block_extract: 0,
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
                aspect_num: 4,
                aspect_den: 3,
            },
            timing: SystemTiming {
                fps: 60.0,
                sample_rate: 44100.0,
            },
        };
    }
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
        let Some(m) = guard.as_mut() else { return };
        let _ = m.run_frame();
        let fb = m.framebuffer();
        if let Some(cb) = unsafe { CALLBACKS.video_refresh } {
            cb(
                fb.as_ptr() as *const c_void,
                FB_WIDTH as c_uint,
                FB_HEIGHT as c_uint,
                FB_WIDTH * 4,
            );
        }
        let audio = m.drain_audio();
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
    let Some(m) = guard.as_mut() else { return };
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
    m.set_input(b);
}

#[no_mangle]
pub extern "C" fn retro_serialize_size() -> usize {
    if let Ok(guard) = CORE.lock() {
        if let Some(m) = guard.as_ref() {
            return m.save_state().len();
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
        if let Some(m) = guard.as_ref() {
            let blob = m.save_state();
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
        if let Some(m) = guard.as_mut() {
            return m.load_state(&buf).is_ok();
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
    let cfg = MachineConfig {
        allow_placeholder_bios: true,
        enable_xa_stream: true,
        audio_test_tone: false,
    };
    let mut m = Machine::new(cfg);
    if !game.is_null() {
        let g = unsafe { &*game };
        if !g.path.is_null() {
            let path = unsafe {
                let mut len = 0usize;
                while *g.path.add(len) != 0 {
                    len += 1;
                }
                std::slice::from_raw_parts(g.path as *const u8, len)
            };
            if let Ok(s) = std::str::from_utf8(path) {
                if m.load_disc_path(std::path::Path::new(s)).is_err() {
                    return false;
                }
            }
        } else if !g.data.is_null() && g.size > 0 {
            let bytes = unsafe { std::slice::from_raw_parts(g.data as *const u8, g.size).to_vec() };
            if m.load_disc_bytes(bytes).is_err() {
                return false;
            }
        }
    }
    m.reset();
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
    *CORE.lock().unwrap() = Some(m);
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
    if id == RETRO_MEMORY_SAVE_RAM {
        0
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    static ACCEPT_FORMAT: AtomicBool = AtomicBool::new(false);
    static VIDEO_SEEN: AtomicBool = AtomicBool::new(false);

    extern "C" fn environment(cmd: c_uint, data: *mut c_void) -> bool {
        if cmd == RETRO_ENVIRONMENT_SET_PIXEL_FORMAT {
            let format = unsafe { *(data as *const c_int) };
            return format == 1 && ACCEPT_FORMAT.load(Ordering::SeqCst);
        }
        true
    }

    extern "C" fn video(data: *const c_void, width: c_uint, height: c_uint, pitch: usize) {
        let pixels =
            unsafe { std::slice::from_raw_parts(data as *const u32, FB_WIDTH * FB_HEIGHT) };
        VIDEO_SEEN.store(
            width == 320
                && height == 240
                && pitch == 320 * 4
                && pixels[0] == 0x0081_8283
                && pixels[320 * 240 - 1] == 0x0001_FE07,
            Ordering::SeqCst,
        );
    }

    #[test]
    fn negotiates_xrgb8888_and_preserves_channels_and_pitch() {
        retro_set_environment(environment);
        retro_set_video_refresh(video);
        assert!(!retro_load_game(ptr::null()));
        assert!(CORE.lock().unwrap().is_none());
        ACCEPT_FORMAT.store(true, Ordering::SeqCst);
        assert!(retro_load_game(ptr::null()));
        {
            let mut guard = CORE.lock().unwrap();
            let m = guard.as_mut().unwrap();
            m.video.framebuffer[0] = 0x0081_8283;
            m.video.framebuffer[320 * 240 - 1] = 0x0001_FE07;
        }
        retro_run();
        assert!(VIDEO_SEEN.load(Ordering::SeqCst));
        retro_unload_game();
    }
}
