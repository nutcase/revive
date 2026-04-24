use super::{
    DmaTarget, FRAME_HEIGHT, FRAME_WIDTH, STATUS_ODD_FRAME, STATUS_SPRITE_COLLISION,
    STATUS_SPRITE_OVERFLOW, STATUS_VBLANK, Vdp, encode_md_color,
};

#[path = "vdp_tests/dma.rs"]
mod dma;
#[path = "vdp_tests/interrupts_timing.rs"]
mod interrupts_timing;
#[path = "vdp_tests/planes_scroll_window.rs"]
mod planes_scroll_window;
#[path = "vdp_tests/ports_memory.rs"]
mod ports_memory;
#[path = "vdp_tests/shadow_highlight.rs"]
mod shadow_highlight;
#[path = "vdp_tests/sprites.rs"]
mod sprites;
