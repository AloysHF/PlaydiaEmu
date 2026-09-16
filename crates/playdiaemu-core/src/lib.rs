//! Deterministic headless Playdia core.
//!
//! Host I/O (windows, audio devices, path pickers) lives outside this crate.

pub mod ac_tables;
pub mod audio;
pub mod bitstream;
pub mod bus;
pub mod cd;
pub mod cdx;
pub mod content;
pub mod diagnostics;
pub mod input;
pub mod machine;
pub mod player;
pub mod sh1;
pub mod state;
pub mod video;

pub use content::{DiscImage, DiscKind, LoadError};
pub use diagnostics::Diagnostics;
pub use input::{InputButtons, InputState};
pub use machine::{Machine, MachineConfig, RunStop};
pub use player::{DiscPlayer, PlayerStop};
pub use state::{SaveStateError, STATE_MAGIC, STATE_VERSION};

/// Native framebuffer geometry produced by the core.
pub const FB_WIDTH: usize = 320;
pub const FB_HEIGHT: usize = 240;

/// Main SH-1 clock used for budget accounting (not yet cycle-accurate).
pub const MAIN_CLOCK_HZ: u32 = 20_000_000;

/// Approximate CPU cycles per emitted video frame (60 Hz).
pub const CYCLES_PER_FRAME: u32 = MAIN_CLOCK_HZ / 60;
