#![allow(static_mut_refs)]
// Logging controls (runtime via env - see debug_flags)
pub(crate) const IMPORTANT_WRITE_LIMIT: u32 = 10; // How many important writes to print

mod access;
mod api;
mod diagnostics;
mod framebuffer;
mod latch;
mod lifecycle;
mod palette;
mod registers;
mod rendering;
mod sprites;
mod state_io;
mod step;
mod superfx_bridge;
mod timing;
mod trace;
mod types;
mod window;

#[cfg(test)]
mod tests;

pub(crate) use trace::{
    trace_cgram_write_config, trace_cgram_write_match, trace_sample_dot_config,
    trace_scanline_state_config, trace_vram_write_config, trace_vram_write_match,
};
pub use types::Ppu;
pub(crate) use types::{BgMapCache, BgRowCache, SpriteData, SpriteSize, WindowLutConfig};
