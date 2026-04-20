use super::{PpuControl, PpuMask, PpuStatus};

pub struct Ppu {
    pub(in crate::ppu) control: PpuControl,
    pub(in crate::ppu) mask: PpuMask,
    pub(in crate::ppu) status: PpuStatus,
    pub(in crate::ppu) oam_addr: u8,
    #[cfg(test)]
    pub v: u16,
    #[cfg(not(test))]
    pub(in crate::ppu) v: u16,
    pub(in crate::ppu) t: u16,
    pub(in crate::ppu) x: u8,
    pub(in crate::ppu) w: bool,
    pub(in crate::ppu) cycle: u16,
    pub(in crate::ppu) scanline: i16,
    pub(in crate::ppu) frame: u64,

    #[cfg(test)]
    pub nametable: [[u8; 1024]; 2],
    #[cfg(not(test))]
    pub(in crate::ppu) nametable: [[u8; 1024]; 2],

    pub(in crate::ppu) palette: [u8; 32],
    pub(in crate::ppu) oam: [u8; 256],
    pub(in crate::ppu) buffer: Vec<u8>,
    pub(in crate::ppu) read_buffer: u8,
    pub(in crate::ppu) nmi_suppressed: bool,
    pub(in crate::ppu) vblank_flag_set_this_frame: bool,
    pub(in crate::ppu) pending_nmi: bool,
    pub frame_complete: bool,
    pub(in crate::ppu) rendering_enabled: bool,
    pub(in crate::ppu) scanline_sprites: [(u8, u8, u8, u8, u8); 8],
    pub(in crate::ppu) scanline_sprite_count: u8,
    pub(in crate::ppu) cached_tile_addr: u16,
    pub(in crate::ppu) cached_tile_low: u8,
    pub(in crate::ppu) cached_tile_high: u8,
    pub(in crate::ppu) cached_nt_map: [u8; 4],
    pub(in crate::ppu) scanline_bg_enable: bool,
    pub(in crate::ppu) scanline_sprite_enable: bool,
    pub(in crate::ppu) scanline_bg_left: bool,
    pub(in crate::ppu) scanline_sprite_left: bool,
    pub(in crate::ppu) scanline_grayscale: bool,
    pub(in crate::ppu) scanline_color_emphasis: u8,
    pub(in crate::ppu) cached_sprite_size: u8,
    pub(in crate::ppu) cached_sprite_pattern_table: u16,
    pub mapper_irq_clock: bool,
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

impl Ppu {
    pub fn new() -> Self {
        Ppu {
            control: PpuControl::empty(),
            mask: PpuMask::empty(),
            // NES-accurate: VBlank is often set at power-on.
            status: PpuStatus::VBLANK,
            oam_addr: 0,
            v: 0,
            t: 0,
            x: 0,
            w: false,
            cycle: 0,
            scanline: -1,
            frame: 0,
            nametable: [[0; 1024]; 2],
            palette: [0x0F; 32],
            oam: [0xFF; 256],
            buffer: {
                let mut buf = Vec::new();
                for _ in 0..(256 * 240) {
                    buf.push(5);
                    buf.push(5);
                    buf.push(5);
                }
                buf
            },
            read_buffer: 0,
            nmi_suppressed: false,
            vblank_flag_set_this_frame: false,
            pending_nmi: false,
            frame_complete: false,
            rendering_enabled: false,
            scanline_sprites: [(0, 0, 0, 0, 0); 8],
            scanline_sprite_count: 0,
            cached_tile_addr: 0xFFFF,
            cached_tile_low: 0,
            cached_tile_high: 0,
            cached_nt_map: [0, 1, 0, 1],
            scanline_bg_enable: false,
            scanline_sprite_enable: false,
            scanline_bg_left: false,
            scanline_sprite_left: false,
            scanline_grayscale: false,
            scanline_color_emphasis: 0,
            cached_sprite_size: 8,
            cached_sprite_pattern_table: 0,
            mapper_irq_clock: false,
        }
    }

    pub fn get_buffer(&self) -> &[u8] {
        &self.buffer
    }
}
