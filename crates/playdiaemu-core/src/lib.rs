//! Deterministic headless Playdia HLE core.
//!
//! Host I/O (windows, audio devices, path pickers) lives outside this crate.

pub mod ac_tables;
pub mod audio;
pub mod bitstream;
pub mod cd;
pub mod content;
pub mod input;
pub mod player;
pub mod state;
pub mod video;

pub use content::{DiscImage, DiscKind, LoadError};
pub use input::{InputButtons, InputState};
pub use player::{DiscPlayer, PlayerStop};
pub use state::{SaveStateError, STATE_MAGIC, STATE_VERSION};

/// Native decoded framebuffer geometry produced by the core.
pub const FB_WIDTH: usize = video::ak8000::WIDTH;
pub const FB_HEIGHT: usize = video::ak8000::HEIGHT;
