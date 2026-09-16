//! Playdia libretro core 鈥?thin C ABI shell over the HLE DiscPlayer.

#![allow(dead_code)]
#![allow(static_mut_refs)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

mod api;
mod callbacks;
mod constants;
mod logger;
mod types;

use playdiaemu_core::player::DiscPlayer;
use std::sync::Mutex;

pub(crate) static CORE: Mutex<Option<DiscPlayer>> = Mutex::new(None);
