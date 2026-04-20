use super::{Ppu, PpuControl, PpuMask, PpuStatus};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PpuRegisterState {
    pub control: u8,
    pub mask: u8,
    pub status: u8,
    pub oam_addr: u8,
    pub v: u16,
    pub t: u16,
    pub x: u8,
    pub w: bool,
    pub scanline: i16,
    pub cycle: u16,
    pub frame: u64,
    pub read_buffer: u8,
}

impl Ppu {
    pub fn get_oam_addr(&self) -> u8 {
        self.oam_addr
    }

    pub fn get_palette(&self) -> [u8; 32] {
        self.palette
    }

    pub fn get_nametable(&self) -> [[u8; 1024]; 2] {
        self.nametable
    }

    pub fn get_oam(&self) -> [u8; 256] {
        self.oam
    }

    pub fn register_state(&self) -> PpuRegisterState {
        PpuRegisterState {
            control: self.control.bits(),
            mask: self.mask.bits(),
            status: self.status.bits(),
            oam_addr: self.oam_addr,
            v: self.v,
            t: self.t,
            x: self.x,
            w: self.w,
            scanline: self.scanline,
            cycle: self.cycle,
            frame: self.frame,
            read_buffer: self.read_buffer,
        }
    }

    // Save state setters
    pub fn set_palette(&mut self, palette: [u8; 32]) {
        self.palette = palette;
    }

    pub fn set_nametable(&mut self, nametable: [[u8; 1024]; 2]) {
        self.nametable = nametable;
    }

    pub fn set_oam(&mut self, oam: [u8; 256]) {
        self.oam = oam;
    }

    pub fn restore_registers(&mut self, state: PpuRegisterState) {
        self.control = PpuControl::from_bits_truncate(state.control);
        self.mask = PpuMask::from_bits_truncate(state.mask);
        self.rendering_enabled =
            self.mask.contains(PpuMask::BG_ENABLE) || self.mask.contains(PpuMask::SPRITE_ENABLE);
        self.status = PpuStatus::from_bits_truncate(state.status);
        self.oam_addr = state.oam_addr;
        self.v = state.v;
        self.t = state.t;
        self.x = state.x;
        self.w = state.w;
        self.scanline = state.scanline;
        self.cycle = state.cycle;
        self.frame = state.frame;
        self.read_buffer = state.read_buffer;
        // Reset scanline caches so they are refreshed on next visible scanline
        self.cached_tile_addr = 0xFFFF;
        self.scanline_bg_enable = self.mask.contains(PpuMask::BG_ENABLE);
        self.scanline_sprite_enable = self.mask.contains(PpuMask::SPRITE_ENABLE);
        self.scanline_bg_left = self.mask.contains(PpuMask::BG_LEFT_ENABLE);
        self.scanline_sprite_left = self.mask.contains(PpuMask::SPRITE_LEFT_ENABLE);
        self.scanline_grayscale = self.mask.contains(PpuMask::GRAYSCALE);
        self.scanline_color_emphasis = self.mask.bits() & 0xE0;
        self.cached_sprite_size = if self.control.contains(PpuControl::SPRITE_SIZE) {
            16
        } else {
            8
        };
        self.cached_sprite_pattern_table = if self.control.contains(PpuControl::SPRITE_PATTERN) {
            0x1000
        } else {
            0x0000
        };
    }

    pub fn write_oam_data(&mut self, addr: u8, data: u8) {
        self.oam[addr as usize] = data;
    }
}
