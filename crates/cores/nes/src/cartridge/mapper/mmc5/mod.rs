use std::cell::Cell;

mod audio;
mod ppu;
mod prg;
mod registers;

pub(in crate::cartridge) use audio::Mmc5Pulse;

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Mmc5 {
    pub(in crate::cartridge) prg_mode: u8,
    pub(in crate::cartridge) chr_mode: u8,
    pub(in crate::cartridge) exram_mode: u8,
    pub(in crate::cartridge) prg_ram_protect_1: u8,
    pub(in crate::cartridge) prg_ram_protect_2: u8,
    pub(in crate::cartridge) nametable_map: [u8; 4],
    pub(in crate::cartridge) fill_tile: u8,
    pub(in crate::cartridge) fill_attr: u8,
    pub(in crate::cartridge) prg_ram_bank: u8,
    pub(in crate::cartridge) prg_banks: [u8; 4],
    pub(in crate::cartridge) chr_upper: u8,
    pub(in crate::cartridge) sprite_chr_banks: [u8; 8],
    pub(in crate::cartridge) bg_chr_banks: [u8; 4],
    pub(in crate::cartridge) exram: Vec<u8>,
    pub(in crate::cartridge) irq_scanline_compare: u8,
    pub(in crate::cartridge) irq_enabled: bool,
    pub(in crate::cartridge) irq_pending: Cell<bool>,
    pub(in crate::cartridge) in_frame: Cell<bool>,
    pub(in crate::cartridge) scanline_counter: Cell<u8>,
    pub(in crate::cartridge) multiplier_a: u8,
    pub(in crate::cartridge) multiplier_b: u8,
    pub(in crate::cartridge) split_control: u8,
    pub(in crate::cartridge) split_scroll: u8,
    pub(in crate::cartridge) split_bank: u8,
    pub(in crate::cartridge) ppu_ctrl: Cell<u8>,
    pub(in crate::cartridge) ppu_mask: Cell<u8>,
    pub(in crate::cartridge) cached_tile_x: Cell<u8>,
    pub(in crate::cartridge) cached_tile_y: Cell<u8>,
    pub(in crate::cartridge) cached_ext_palette: Cell<u8>,
    pub(in crate::cartridge) cached_ext_bank: Cell<u8>,
    pub(in crate::cartridge) ppu_data_uses_bg_banks: bool,
    pub(in crate::cartridge) pulse1: Mmc5Pulse,
    pub(in crate::cartridge) pulse2: Mmc5Pulse,
    pub(in crate::cartridge) pulse1_enabled: bool,
    pub(in crate::cartridge) pulse2_enabled: bool,
    pub(in crate::cartridge) pcm_irq_enabled: bool,
    pub(in crate::cartridge) pcm_read_mode: bool,
    pub(in crate::cartridge) pcm_irq_pending: Cell<bool>,
    pub(in crate::cartridge) pcm_dac: u8,
    pub(in crate::cartridge) audio_frame_accum: u32,
    pub(in crate::cartridge) audio_even_cycle: bool,
}

impl Mmc5 {
    pub(in crate::cartridge) fn new() -> Self {
        Self {
            prg_mode: 3,
            chr_mode: 3,
            exram_mode: 0,
            prg_ram_protect_1: 0,
            prg_ram_protect_2: 0,
            nametable_map: [0, 1, 0, 1],
            fill_tile: 0,
            fill_attr: 0,
            prg_ram_bank: 0,
            prg_banks: [0x00, 0x80, 0x80, 0x7F],
            chr_upper: 0,
            sprite_chr_banks: [0; 8],
            bg_chr_banks: [0; 4],
            exram: vec![0; 1024],
            irq_scanline_compare: 0,
            irq_enabled: false,
            irq_pending: Cell::new(false),
            in_frame: Cell::new(false),
            scanline_counter: Cell::new(0),
            multiplier_a: 0xFF,
            multiplier_b: 0xFF,
            split_control: 0,
            split_scroll: 0,
            split_bank: 0,
            ppu_ctrl: Cell::new(0),
            ppu_mask: Cell::new(0),
            cached_tile_x: Cell::new(0),
            cached_tile_y: Cell::new(0),
            cached_ext_palette: Cell::new(0),
            cached_ext_bank: Cell::new(0),
            ppu_data_uses_bg_banks: false,
            pulse1: Mmc5Pulse::new(),
            pulse2: Mmc5Pulse::new(),
            pulse1_enabled: false,
            pulse2_enabled: false,
            pcm_irq_enabled: false,
            pcm_read_mode: true,
            pcm_irq_pending: Cell::new(false),
            pcm_dac: 0,
            audio_frame_accum: 0,
            audio_even_cycle: false,
        }
    }

    fn substitutions_enabled(&self) -> bool {
        self.ppu_mask.get() & 0x18 != 0
    }

    fn prg_ram_write_enabled(&self) -> bool {
        self.prg_ram_protect_1 == 0x02 && self.prg_ram_protect_2 == 0x01
    }

    fn split_enabled(&self) -> bool {
        self.substitutions_enabled() && self.exram_mode <= 0x01 && self.split_control & 0x80 != 0
    }

    fn split_on_right(&self) -> bool {
        self.split_control & 0x40 != 0
    }

    fn split_threshold_tiles(&self) -> usize {
        (self.split_control & 0x1F) as usize
    }

    pub(in crate::cartridge) fn combined_irq_pending(&self) -> bool {
        self.irq_pending.get() || (self.pcm_irq_enabled && self.pcm_irq_pending.get())
    }
}
