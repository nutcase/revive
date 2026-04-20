use crate::apu::GbApu;
use crate::cartridge::{GbCartridge, GbCartridgeHeader};
use emulator_core::EmuResult;

const VRAM_BANK_SIZE: usize = 8 * 1024;
const VRAM_BANKS: usize = 2;
const VRAM_SIZE: usize = VRAM_BANK_SIZE * VRAM_BANKS;
const WRAM_BANK_SIZE: usize = 4 * 1024;
const WRAM_BANKS: usize = 8;
const WRAM_SIZE: usize = WRAM_BANK_SIZE * WRAM_BANKS;
const OAM_SIZE: usize = 160;
const IO_SIZE: usize = 128;
const HRAM_SIZE: usize = 127;

const JOYP_INDEX: usize = 0x00;
const SB_INDEX: usize = 0x01;
const SC_INDEX: usize = 0x02;
const IF_INDEX: usize = 0x0F;
const DIV_INDEX: usize = 0x04;
const TIMA_INDEX: usize = 0x05;
const TMA_INDEX: usize = 0x06;
const TAC_INDEX: usize = 0x07;
const LCDC_INDEX: usize = 0x40;
const STAT_INDEX: usize = 0x41;
const SCY_INDEX: usize = 0x42;
const SCX_INDEX: usize = 0x43;
const LY_INDEX: usize = 0x44;
const LYC_INDEX: usize = 0x45;
const BGP_INDEX: usize = 0x47;
const OBP0_INDEX: usize = 0x48;
const OBP1_INDEX: usize = 0x49;
const WY_INDEX: usize = 0x4A;
const WX_INDEX: usize = 0x4B;
const KEY1_INDEX: usize = 0x4D;
const VBK_INDEX: usize = 0x4F;
const HDMA1_INDEX: usize = 0x51;
const HDMA2_INDEX: usize = 0x52;
const HDMA3_INDEX: usize = 0x53;
const HDMA4_INDEX: usize = 0x54;
const HDMA5_INDEX: usize = 0x55;
const BGPI_INDEX: usize = 0x68;
const BGPD_INDEX: usize = 0x69;
const OBPI_INDEX: usize = 0x6A;
const OBPD_INDEX: usize = 0x6B;
const SVBK_INDEX: usize = 0x70;
const CGB_PALETTE_BYTES: usize = 64;

pub const INT_VBLANK: u8 = 1 << 0;
pub const INT_LCD_STAT: u8 = 1 << 1;
pub const INT_TIMER: u8 = 1 << 2;
pub const INT_SERIAL: u8 = 1 << 3;
pub const INT_JOYPAD: u8 = 1 << 4;

pub const GB_KEY_RIGHT: u8 = 1 << 0;
pub const GB_KEY_LEFT: u8 = 1 << 1;
pub const GB_KEY_UP: u8 = 1 << 2;
pub const GB_KEY_DOWN: u8 = 1 << 3;
pub const GB_KEY_A: u8 = 1 << 4;
pub const GB_KEY_B: u8 = 1 << 5;
pub const GB_KEY_SELECT: u8 = 1 << 6;
pub const GB_KEY_START: u8 = 1 << 7;

#[derive(Debug)]
pub struct GbBus {
    cartridge: Option<GbCartridge>,
    vram: [u8; VRAM_SIZE],
    wram: [u8; WRAM_SIZE],
    oam: [u8; OAM_SIZE],
    io: [u8; IO_SIZE],
    hram: [u8; HRAM_SIZE],
    ie: u8,
    div_reset_requested: bool,
    joypad_pressed: u8,
    cgb_mode: bool,
    vram_bank: usize,
    wram_bank: usize,
    cgb_bg_palette_data: [u8; CGB_PALETTE_BYTES],
    cgb_obj_palette_data: [u8; CGB_PALETTE_BYTES],
    cgb_bg_palette_index: u8,
    cgb_obj_palette_index: u8,
    cgb_double_speed: bool,
    cgb_key1_prepare: bool,
    hdma_active: bool,
    hdma_source: u16,
    hdma_dest: u16,
    hdma_blocks_remaining: u8,
    apu: GbApu,
    debug_vram_write_count: u64,
    debug_hdma_bytes_copied: u64,
}

impl Default for GbBus {
    fn default() -> Self {
        Self {
            cartridge: None,
            vram: [0; VRAM_SIZE],
            wram: [0; WRAM_SIZE],
            oam: [0; OAM_SIZE],
            io: [0; IO_SIZE],
            hram: [0; HRAM_SIZE],
            ie: 0,
            div_reset_requested: false,
            joypad_pressed: 0,
            cgb_mode: false,
            vram_bank: 0,
            wram_bank: 1,
            cgb_bg_palette_data: [0; CGB_PALETTE_BYTES],
            cgb_obj_palette_data: [0; CGB_PALETTE_BYTES],
            cgb_bg_palette_index: 0,
            cgb_obj_palette_index: 0,
            cgb_double_speed: false,
            cgb_key1_prepare: false,
            hdma_active: false,
            hdma_source: 0,
            hdma_dest: 0x8000,
            hdma_blocks_remaining: 0,
            apu: GbApu::default(),
            debug_vram_write_count: 0,
            debug_hdma_bytes_copied: 0,
        }
    }
}

impl GbBus {
    pub fn set_cgb_mode(&mut self, enabled: bool) {
        self.cgb_mode = enabled;
        if !enabled {
            self.vram_bank = 0;
            self.wram_bank = 1;
            self.cgb_double_speed = false;
            self.cgb_key1_prepare = false;
        }
        self.io[VBK_INDEX] = 0xFE | (self.vram_bank as u8 & 0x01);
        self.io[SVBK_INDEX] = 0xF8 | ((self.wram_bank as u8) & 0x07);
        self.update_key1_register();
    }

    pub fn cgb_mode(&self) -> bool {
        self.cgb_mode
    }

    pub fn handle_stop(&mut self) {
        if self.cgb_mode && self.cgb_key1_prepare {
            self.cgb_double_speed = !self.cgb_double_speed;
            self.cgb_key1_prepare = false;
            self.update_key1_register();
        }
    }

    pub fn cgb_double_speed(&self) -> bool {
        self.cgb_mode && self.cgb_double_speed
    }

    pub fn step_hblank_hdma(&mut self) {
        if !self.cgb_mode || !self.hdma_active || self.hdma_blocks_remaining == 0 {
            return;
        }
        self.copy_hdma_block();
    }

    pub fn load_cartridge(&mut self, rom: &[u8]) -> EmuResult<GbCartridgeHeader> {
        let cartridge = GbCartridge::from_rom(rom.to_vec())?;
        let header = cartridge.header().clone();
        self.cartridge = Some(cartridge);
        Ok(header)
    }

    pub fn reset(&mut self) {
        self.vram = [0; VRAM_SIZE];
        self.wram = [0; WRAM_SIZE];
        self.oam = [0; OAM_SIZE];
        self.io = [0; IO_SIZE];
        self.io[JOYP_INDEX] = 0xCF;
        self.io[SB_INDEX] = 0x00;
        self.io[SC_INDEX] = 0x7E;
        self.io[0x10] = 0x80;
        self.io[0x11] = 0xBF;
        self.io[0x12] = 0xF3;
        self.io[0x14] = 0xBF;
        self.io[0x16] = 0x3F;
        self.io[0x19] = 0xBF;
        self.io[0x1A] = 0x7F;
        self.io[0x1B] = 0xFF;
        self.io[0x1C] = 0x9F;
        self.io[0x1E] = 0xBF;
        self.io[0x20] = 0xFF;
        self.io[0x23] = 0xBF;
        self.io[0x24] = 0x77;
        self.io[0x25] = 0xF3;
        self.io[0x26] = 0xF1;
        self.io[LCDC_INDEX] = 0x91;
        self.io[STAT_INDEX] = 0x85;
        self.io[SCY_INDEX] = 0x00;
        self.io[SCX_INDEX] = 0x00;
        self.io[LY_INDEX] = 0x00;
        self.io[LYC_INDEX] = 0x00;
        self.io[BGP_INDEX] = 0xFC;
        self.io[OBP0_INDEX] = 0xFF;
        self.io[OBP1_INDEX] = 0xFF;
        self.io[WY_INDEX] = 0x00;
        self.io[WX_INDEX] = 0x00;
        self.io[KEY1_INDEX] = 0x7E;
        self.io[VBK_INDEX] = 0xFE;
        self.io[HDMA1_INDEX] = 0xFF;
        self.io[HDMA2_INDEX] = 0xFF;
        self.io[HDMA3_INDEX] = 0xFF;
        self.io[HDMA4_INDEX] = 0xFF;
        self.io[HDMA5_INDEX] = 0xFF;
        self.io[BGPI_INDEX] = 0x00;
        self.io[OBPI_INDEX] = 0x00;
        self.io[SVBK_INDEX] = 0xF9;
        self.hram = [0; HRAM_SIZE];
        self.ie = 0;
        self.div_reset_requested = false;
        self.joypad_pressed = 0;
        self.vram_bank = 0;
        self.wram_bank = 1;
        if self.cgb_mode {
            self.cgb_bg_palette_data = [0; CGB_PALETTE_BYTES];
            self.cgb_obj_palette_data = [0; CGB_PALETTE_BYTES];
            for i in (0..CGB_PALETTE_BYTES).step_by(2) {
                self.cgb_bg_palette_data[i] = 0xFF;
                self.cgb_bg_palette_data[i + 1] = 0x7F;
                self.cgb_obj_palette_data[i] = 0xFF;
                self.cgb_obj_palette_data[i + 1] = 0x7F;
            }
        } else {
            self.cgb_bg_palette_data = [0; CGB_PALETTE_BYTES];
            self.cgb_obj_palette_data = [0; CGB_PALETTE_BYTES];
        }
        self.cgb_bg_palette_index = 0;
        self.cgb_obj_palette_index = 0;
        self.cgb_double_speed = false;
        self.cgb_key1_prepare = false;
        self.hdma_active = false;
        self.hdma_source = 0;
        self.hdma_dest = 0x8000;
        self.hdma_blocks_remaining = 0;
        self.apu.reset(&mut self.io);
        self.update_key1_register();
        self.debug_vram_write_count = 0;
        self.debug_hdma_bytes_copied = 0;

        if let Some(cartridge) = self.cartridge.as_mut() {
            cartridge.reset();
        }
    }

    pub fn read8(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self
                .cartridge
                .as_ref()
                .map(|cart| cart.read_rom(addr))
                .unwrap_or(0xFF),
            0x8000..=0x9FFF => self.read_vram_cpu(addr),
            0xA000..=0xBFFF => self
                .cartridge
                .as_ref()
                .map(|cart| cart.read_ram(addr))
                .unwrap_or(0xFF),
            0xC000..=0xCFFF => self.wram[(addr as usize) - 0xC000],
            0xD000..=0xDFFF => {
                self.wram[self.wram_bank * WRAM_BANK_SIZE + (addr as usize - 0xD000)]
            }
            0xE000..=0xFDFF => self.read8(addr.wrapping_sub(0x2000)),
            0xFE00..=0xFE9F => self.oam[(addr as usize) - 0xFE00],
            0xFEA0..=0xFEFF => 0xFF,
            0xFF00..=0xFF7F => {
                let index = (addr as usize) - 0xFF00;
                if index == JOYP_INDEX {
                    self.read_joyp()
                } else if index == IF_INDEX {
                    self.io[index] | 0xE0
                } else if index == STAT_INDEX {
                    self.io[index] | 0x80
                } else if index == KEY1_INDEX {
                    self.io[KEY1_INDEX]
                } else if index == VBK_INDEX {
                    0xFE | (self.vram_bank as u8 & 0x01)
                } else if index == BGPI_INDEX {
                    self.cgb_bg_palette_index | 0x40
                } else if index == BGPD_INDEX {
                    self.cgb_bg_palette_data[usize::from(self.cgb_bg_palette_index & 0x3F)]
                } else if index == OBPI_INDEX {
                    self.cgb_obj_palette_index | 0x40
                } else if index == OBPD_INDEX {
                    self.cgb_obj_palette_data[usize::from(self.cgb_obj_palette_index & 0x3F)]
                } else if index == SVBK_INDEX {
                    0xF8 | ((self.wram_bank as u8) & 0x07)
                } else if GbApu::is_apu_register(index) {
                    self.apu.read_reg(index, &self.io)
                } else {
                    self.io[index]
                }
            }
            0xFF80..=0xFFFE => self.hram[(addr as usize) - 0xFF80],
            0xFFFF => self.ie,
        }
    }

    pub fn read16(&self, addr: u16) -> u16 {
        let low = self.read8(addr) as u16;
        let high = self.read8(addr.wrapping_add(1)) as u16;
        low | (high << 8)
    }

    pub fn write8(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x7FFF => {
                if let Some(cartridge) = self.cartridge.as_mut() {
                    cartridge.write_rom_control(addr, value);
                }
            }
            0x8000..=0x9FFF => self.write_vram_cpu(addr, value),
            0xA000..=0xBFFF => {
                if let Some(cartridge) = self.cartridge.as_mut() {
                    cartridge.write_ram(addr, value);
                }
            }
            0xC000..=0xCFFF => self.wram[(addr as usize) - 0xC000] = value,
            0xD000..=0xDFFF => {
                let index = self.wram_bank * WRAM_BANK_SIZE + (addr as usize - 0xD000);
                self.wram[index] = value;
            }
            0xE000..=0xFDFF => self.write8(addr.wrapping_sub(0x2000), value),
            0xFE00..=0xFE9F => self.oam[(addr as usize) - 0xFE00] = value,
            0xFEA0..=0xFEFF => {}
            0xFF00..=0xFF7F => {
                let index = (addr as usize) - 0xFF00;
                match index {
                    JOYP_INDEX => {
                        let before = self.read_joyp() & 0x0F;
                        self.io[JOYP_INDEX] = 0xC0 | (value & 0x30) | 0x0F;
                        let after = self.read_joyp() & 0x0F;
                        if (before & !after) != 0 {
                            self.request_interrupt(INT_JOYPAD);
                        }
                    }
                    DIV_INDEX => self.io[DIV_INDEX] = 0,
                    TAC_INDEX => self.io[TAC_INDEX] = value & 0x07,
                    IF_INDEX => self.io[IF_INDEX] = value & 0x1F,
                    STAT_INDEX => {
                        self.io[STAT_INDEX] = 0x80 | (value & 0x78) | (self.io[STAT_INDEX] & 0x07);
                    }
                    LY_INDEX => self.io[LY_INDEX] = 0,
                    0x46 => {
                        self.io[0x46] = value;
                        let source_base = u16::from(value) << 8;
                        let mut dma_buffer = [0u8; OAM_SIZE];
                        for (offset, byte) in dma_buffer.iter_mut().enumerate() {
                            *byte = self.read8(source_base.wrapping_add(offset as u16));
                        }
                        self.oam.copy_from_slice(&dma_buffer);
                    }
                    KEY1_INDEX => {
                        if self.cgb_mode {
                            self.cgb_key1_prepare = (value & 0x01) != 0;
                        } else {
                            self.cgb_key1_prepare = false;
                        }
                        self.update_key1_register();
                    }
                    VBK_INDEX => {
                        if self.cgb_mode {
                            self.vram_bank = usize::from(value & 0x01);
                        }
                        self.io[VBK_INDEX] = 0xFE | (self.vram_bank as u8 & 0x01);
                    }
                    HDMA1_INDEX => {
                        self.io[HDMA1_INDEX] = value;
                    }
                    HDMA2_INDEX => {
                        self.io[HDMA2_INDEX] = value & 0xF0;
                    }
                    HDMA3_INDEX => {
                        self.io[HDMA3_INDEX] = value & 0x1F;
                    }
                    HDMA4_INDEX => {
                        self.io[HDMA4_INDEX] = value & 0xF0;
                    }
                    HDMA5_INDEX => {
                        if !self.cgb_mode {
                            self.io[HDMA5_INDEX] = 0xFF;
                        } else if self.hdma_active {
                            if (value & 0x80) == 0 {
                                self.hdma_active = false;
                                let remain = self.hdma_blocks_remaining.saturating_sub(1) & 0x7F;
                                self.io[HDMA5_INDEX] = 0x80 | remain;
                            }
                        } else if (value & 0x80) == 0 {
                            self.execute_gdma(value);
                        } else {
                            self.start_hblank_hdma(value);
                        }
                    }
                    BGPI_INDEX => {
                        self.cgb_bg_palette_index = value & 0xBF;
                        self.io[BGPI_INDEX] = self.cgb_bg_palette_index;
                    }
                    BGPD_INDEX => {
                        let idx = usize::from(self.cgb_bg_palette_index & 0x3F);
                        self.cgb_bg_palette_data[idx] = value;
                        if (self.cgb_bg_palette_index & 0x80) != 0 {
                            let next = (self.cgb_bg_palette_index.wrapping_add(1)) & 0x3F;
                            self.cgb_bg_palette_index = next | 0x80;
                            self.io[BGPI_INDEX] = self.cgb_bg_palette_index;
                        }
                    }
                    OBPI_INDEX => {
                        self.cgb_obj_palette_index = value & 0xBF;
                        self.io[OBPI_INDEX] = self.cgb_obj_palette_index;
                    }
                    OBPD_INDEX => {
                        let idx = usize::from(self.cgb_obj_palette_index & 0x3F);
                        self.cgb_obj_palette_data[idx] = value;
                        if (self.cgb_obj_palette_index & 0x80) != 0 {
                            let next = (self.cgb_obj_palette_index.wrapping_add(1)) & 0x3F;
                            self.cgb_obj_palette_index = next | 0x80;
                            self.io[OBPI_INDEX] = self.cgb_obj_palette_index;
                        }
                    }
                    SVBK_INDEX => {
                        if self.cgb_mode {
                            let bank = usize::from(value & 0x07).max(1);
                            self.wram_bank = bank.min(7);
                        } else {
                            self.wram_bank = 1;
                        }
                        self.io[SVBK_INDEX] = 0xF8 | ((self.wram_bank as u8) & 0x07);
                    }
                    idx if GbApu::is_apu_register(idx) => {
                        self.apu.write_reg(idx, value, &mut self.io);
                    }
                    _ => self.io[index] = value,
                }
                if index == DIV_INDEX {
                    self.div_reset_requested = true;
                }
            }
            0xFF80..=0xFFFE => self.hram[(addr as usize) - 0xFF80] = value,
            0xFFFF => self.ie = value,
        }
    }

    pub fn write16(&mut self, addr: u16, value: u16) {
        self.write8(addr, (value & 0x00FF) as u8);
        self.write8(addr.wrapping_add(1), (value >> 8) as u8);
    }

    pub fn timer_tima(&self) -> u8 {
        self.io[TIMA_INDEX]
    }

    pub fn timer_tma(&self) -> u8 {
        self.io[TMA_INDEX]
    }

    pub fn timer_tac(&self) -> u8 {
        self.io[TAC_INDEX] & 0x07
    }

    pub fn serial_sc(&self) -> u8 {
        self.io[SC_INDEX]
    }

    pub fn set_serial_sb(&mut self, value: u8) {
        self.io[SB_INDEX] = value;
    }

    pub fn set_serial_sc(&mut self, value: u8) {
        self.io[SC_INDEX] = value;
    }

    pub fn ppu_lcdc(&self) -> u8 {
        self.io[LCDC_INDEX]
    }

    pub fn ppu_stat(&self) -> u8 {
        self.io[STAT_INDEX] | 0x80
    }

    pub fn ppu_ly(&self) -> u8 {
        self.io[LY_INDEX]
    }

    pub fn ppu_lyc(&self) -> u8 {
        self.io[LYC_INDEX]
    }

    pub fn ppu_scx(&self) -> u8 {
        self.io[SCX_INDEX]
    }

    pub fn ppu_scy(&self) -> u8 {
        self.io[SCY_INDEX]
    }

    pub fn ppu_bg_palette(&self) -> u8 {
        self.io[BGP_INDEX]
    }

    pub fn ppu_obj_palette0(&self) -> u8 {
        self.io[OBP0_INDEX]
    }

    pub fn ppu_obj_palette1(&self) -> u8 {
        self.io[OBP1_INDEX]
    }

    pub fn ppu_wx(&self) -> u8 {
        self.io[WX_INDEX]
    }

    pub fn ppu_wy(&self) -> u8 {
        self.io[WY_INDEX]
    }

    pub fn set_timer_div(&mut self, value: u8) {
        self.io[DIV_INDEX] = value;
    }

    pub fn set_timer_tima(&mut self, value: u8) {
        self.io[TIMA_INDEX] = value;
    }

    pub fn request_interrupt(&mut self, mask: u8) {
        self.io[IF_INDEX] = (self.io[IF_INDEX] | (mask & 0x1F)) & 0x1F;
    }

    pub fn acknowledge_interrupt(&mut self, mask: u8) {
        self.io[IF_INDEX] &= !(mask & 0x1F);
    }

    pub fn pending_interrupts(&self) -> u8 {
        (self.io[IF_INDEX] & self.ie) & 0x1F
    }

    pub fn take_div_reset_request(&mut self) -> bool {
        let requested = self.div_reset_requested;
        self.div_reset_requested = false;
        requested
    }

    pub fn set_ppu_ly(&mut self, value: u8) {
        self.io[LY_INDEX] = value;
    }

    pub fn set_ppu_mode(&mut self, mode: u8) {
        self.io[STAT_INDEX] = (self.io[STAT_INDEX] & !0x03) | (mode & 0x03) | 0x80;
    }

    pub fn set_ppu_lyc_flag(&mut self, equal: bool) {
        if equal {
            self.io[STAT_INDEX] |= 0x04;
        } else {
            self.io[STAT_INDEX] &= !0x04;
        }
        self.io[STAT_INDEX] |= 0x80;
    }

    pub fn set_keyinput_pressed_mask(&mut self, pressed_mask: u8) {
        let before = self.read_joyp() & 0x0F;
        self.joypad_pressed = pressed_mask;
        let after = self.read_joyp() & 0x0F;
        if (before & !after) != 0 {
            self.request_interrupt(INT_JOYPAD);
        }
    }

    pub fn ppu_read_vram_bank(&self, bank: u8, addr: u16) -> u8 {
        if !(0x8000..=0x9FFF).contains(&addr) {
            return 0xFF;
        }
        let bank_index = (usize::from(bank & 0x01)) * VRAM_BANK_SIZE;
        self.vram[bank_index + (addr as usize - 0x8000)]
    }

    pub fn ppu_read_oam(&self, offset: usize) -> u8 {
        self.oam.get(offset).copied().unwrap_or(0xFF)
    }

    pub fn cgb_bg_palette_byte(&self, index: u8) -> u8 {
        self.cgb_bg_palette_data[usize::from(index & 0x3F)]
    }

    pub fn cgb_obj_palette_byte(&self, index: u8) -> u8 {
        self.cgb_obj_palette_data[usize::from(index & 0x3F)]
    }

    pub fn debug_vram_write_count(&self) -> u64 {
        self.debug_vram_write_count
    }

    pub fn debug_hdma_bytes_copied(&self) -> u64 {
        self.debug_hdma_bytes_copied
    }

    pub fn mix_audio_for_cycles(&mut self, cycles: u32) {
        self.apu.mix_audio_for_cycles(cycles, &mut self.io);
    }

    pub fn take_audio_samples_i16_into(&mut self, out: &mut Vec<i16>) {
        self.apu.take_audio_samples_i16_into(out);
    }

    pub fn audio_sample_rate_hz(&self) -> u32 {
        self.apu.audio_sample_rate_hz()
    }

    pub fn load_cartridge_ram(&mut self, data: &[u8]) {
        if let Some(cartridge) = self.cartridge.as_mut() {
            cartridge.load_ram_data(data);
        }
    }

    pub fn cartridge_ram_data(&self) -> Option<&[u8]> {
        self.cartridge
            .as_ref()
            .and_then(|cartridge| cartridge.ram_data())
    }

    fn read_joyp(&self) -> u8 {
        let mut value = (self.io[JOYP_INDEX] & 0x30) | 0xC0 | 0x0F;
        let direction_selected = (value & 0x10) == 0;
        let action_selected = (value & 0x20) == 0;

        if direction_selected {
            if (self.joypad_pressed & GB_KEY_RIGHT) != 0 {
                value &= !0x01;
            }
            if (self.joypad_pressed & GB_KEY_LEFT) != 0 {
                value &= !0x02;
            }
            if (self.joypad_pressed & GB_KEY_UP) != 0 {
                value &= !0x04;
            }
            if (self.joypad_pressed & GB_KEY_DOWN) != 0 {
                value &= !0x08;
            }
        }

        if action_selected {
            if (self.joypad_pressed & GB_KEY_A) != 0 {
                value &= !0x01;
            }
            if (self.joypad_pressed & GB_KEY_B) != 0 {
                value &= !0x02;
            }
            if (self.joypad_pressed & GB_KEY_SELECT) != 0 {
                value &= !0x04;
            }
            if (self.joypad_pressed & GB_KEY_START) != 0 {
                value &= !0x08;
            }
        }

        value
    }

    fn read_vram_cpu(&self, addr: u16) -> u8 {
        let index = self.vram_bank * VRAM_BANK_SIZE + (addr as usize - 0x8000);
        self.vram[index]
    }

    fn write_vram_cpu(&mut self, addr: u16, value: u8) {
        let index = self.vram_bank * VRAM_BANK_SIZE + (addr as usize - 0x8000);
        self.vram[index] = value;
        self.debug_vram_write_count = self.debug_vram_write_count.saturating_add(1);
    }

    fn write_vram_bank(&mut self, bank: usize, addr: u16, value: u8) {
        if !(0x8000..=0x9FFF).contains(&addr) {
            return;
        }
        let bank = bank.min(VRAM_BANKS - 1);
        let index = bank * VRAM_BANK_SIZE + (addr as usize - 0x8000);
        self.vram[index] = value;
        self.debug_vram_write_count = self.debug_vram_write_count.saturating_add(1);
    }

    fn execute_gdma(&mut self, control: u8) {
        let blocks = (control & 0x7F).wrapping_add(1);
        let source = self.hdma_source_from_regs();
        let dest = self.hdma_dest_from_regs();
        self.begin_hdma_transfer(source, dest, blocks);
        while self.hdma_active {
            self.copy_hdma_block();
        }
    }

    fn start_hblank_hdma(&mut self, control: u8) {
        let blocks = (control & 0x7F).wrapping_add(1);
        let source = self.hdma_source_from_regs();
        let dest = self.hdma_dest_from_regs();
        self.begin_hdma_transfer(source, dest, blocks);
        self.io[HDMA5_INDEX] = self.hdma_blocks_remaining.wrapping_sub(1) & 0x7F;
    }

    fn begin_hdma_transfer(&mut self, source: u16, dest: u16, blocks: u8) {
        self.hdma_source = source;
        self.hdma_dest = dest;
        self.hdma_blocks_remaining = blocks.max(1);
        self.hdma_active = true;
        self.sync_hdma_regs_from_state();
    }

    fn copy_hdma_block(&mut self) {
        if !self.hdma_active || self.hdma_blocks_remaining == 0 {
            return;
        }

        for offset in 0..0x10usize {
            let source = self.hdma_source.wrapping_add(offset as u16);
            let dest_offset = (usize::from(self.hdma_dest.wrapping_sub(0x8000)) + offset) & 0x1FFF;
            let dest = 0x8000 | (dest_offset as u16);
            let byte = self.read8(source);
            self.write_vram_bank(self.vram_bank, dest, byte);
        }
        self.debug_hdma_bytes_copied = self.debug_hdma_bytes_copied.saturating_add(0x10);

        self.hdma_source = self.hdma_source.wrapping_add(0x10);
        let next_dest_offset = (usize::from(self.hdma_dest.wrapping_sub(0x8000)) + 0x10) & 0x1FF0;
        self.hdma_dest = 0x8000 | (next_dest_offset as u16);
        self.hdma_blocks_remaining = self.hdma_blocks_remaining.saturating_sub(1);
        self.sync_hdma_regs_from_state();

        if self.hdma_blocks_remaining == 0 {
            self.hdma_active = false;
            self.io[HDMA5_INDEX] = 0xFF;
        } else {
            self.io[HDMA5_INDEX] = self.hdma_blocks_remaining.wrapping_sub(1) & 0x7F;
        }
    }

    fn hdma_source_from_regs(&self) -> u16 {
        (u16::from(self.io[HDMA1_INDEX]) << 8) | u16::from(self.io[HDMA2_INDEX] & 0xF0)
    }

    fn hdma_dest_from_regs(&self) -> u16 {
        0x8000
            | ((u16::from(self.io[HDMA3_INDEX] & 0x1F)) << 8)
            | u16::from(self.io[HDMA4_INDEX] & 0xF0)
    }

    fn sync_hdma_regs_from_state(&mut self) {
        self.io[HDMA1_INDEX] = (self.hdma_source >> 8) as u8;
        self.io[HDMA2_INDEX] = (self.hdma_source as u8) & 0xF0;
        self.io[HDMA3_INDEX] = ((self.hdma_dest >> 8) as u8) & 0x1F;
        self.io[HDMA4_INDEX] = (self.hdma_dest as u8) & 0xF0;
    }

    fn update_key1_register(&mut self) {
        let speed = if self.cgb_double_speed { 0x80 } else { 0x00 };
        let prepare = if self.cgb_key1_prepare { 0x01 } else { 0x00 };
        self.io[KEY1_INDEX] = 0x7E | speed | prepare;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_rom() -> Vec<u8> {
        let mut rom = vec![0; 0x8000];
        rom[0x0147] = 0x00;
        rom[0x0148] = 0x00;
        rom[0x0149] = 0x00;
        rom
    }

    #[test]
    fn echo_ram_reflects_work_ram() {
        let mut bus = GbBus::default();
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");

        bus.write8(0xC123, 0x42);
        assert_eq!(bus.read8(0xE123), 0x42);

        bus.write8(0xE456, 0xAA);
        assert_eq!(bus.read8(0xC456), 0xAA);
    }

    #[test]
    fn read16_write16_round_trip() {
        let mut bus = GbBus::default();
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");

        bus.write16(0xC000, 0xBEEF);
        assert_eq!(bus.read16(0xC000), 0xBEEF);
    }

    #[test]
    fn interrupt_request_and_acknowledge() {
        let mut bus = GbBus::default();
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");

        bus.write8(0xFFFF, INT_TIMER);
        bus.request_interrupt(INT_TIMER);
        assert_eq!(bus.pending_interrupts(), INT_TIMER);

        bus.acknowledge_interrupt(INT_TIMER);
        assert_eq!(bus.pending_interrupts(), 0);
    }

    #[test]
    fn cgb_vram_bank_switch_uses_vbk() {
        let mut bus = GbBus::default();
        bus.set_cgb_mode(true);
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        bus.reset();

        bus.write8(0x8000, 0x12);
        assert_eq!(bus.ppu_read_vram_bank(0, 0x8000), 0x12);
        assert_eq!(bus.ppu_read_vram_bank(1, 0x8000), 0x00);

        bus.write8(0xFF4F, 0x01);
        bus.write8(0x8000, 0x34);
        assert_eq!(bus.ppu_read_vram_bank(0, 0x8000), 0x12);
        assert_eq!(bus.ppu_read_vram_bank(1, 0x8000), 0x34);
    }

    #[test]
    fn cgb_palette_data_write_reads_back() {
        let mut bus = GbBus::default();
        bus.set_cgb_mode(true);
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        bus.reset();

        bus.write8(0xFF68, 0x80);
        bus.write8(0xFF69, 0x1F);
        bus.write8(0xFF69, 0x03);

        assert_eq!(bus.cgb_bg_palette_byte(0), 0x1F);
        assert_eq!(bus.cgb_bg_palette_byte(1), 0x03);
    }

    #[test]
    fn cgb_hdma_copies_block_to_vram() {
        let mut bus = GbBus::default();
        bus.set_cgb_mode(true);
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        bus.reset();

        for i in 0..16u8 {
            bus.write8(0xC000 + u16::from(i), i.wrapping_mul(3));
        }

        bus.write8(0xFF51, 0xC0);
        bus.write8(0xFF52, 0x00);
        bus.write8(0xFF53, 0x00);
        bus.write8(0xFF54, 0x00);
        bus.write8(0xFF55, 0x00);

        for i in 0..16u8 {
            assert_eq!(
                bus.ppu_read_vram_bank(0, 0x8000 + u16::from(i)),
                i.wrapping_mul(3)
            );
        }
        assert_eq!(bus.read8(0xFF55), 0xFF);
    }

    #[test]
    fn cgb_hblank_hdma_copies_one_block_per_step() {
        let mut bus = GbBus::default();
        bus.set_cgb_mode(true);
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        bus.reset();

        for i in 0..32u8 {
            bus.write8(0xC100 + u16::from(i), i.wrapping_add(0x40));
        }

        bus.write8(0xFF51, 0xC1);
        bus.write8(0xFF52, 0x00);
        bus.write8(0xFF53, 0x00);
        bus.write8(0xFF54, 0x00);
        bus.write8(0xFF55, 0x81);

        for i in 0..16u8 {
            assert_eq!(bus.ppu_read_vram_bank(0, 0x8000 + u16::from(i)), 0x00);
        }
        assert_eq!(bus.read8(0xFF55) & 0x7F, 0x01);

        bus.step_hblank_hdma();
        for i in 0..16u8 {
            assert_eq!(
                bus.ppu_read_vram_bank(0, 0x8000 + u16::from(i)),
                i.wrapping_add(0x40)
            );
        }
        assert_eq!(bus.read8(0xFF55) & 0x7F, 0x00);

        bus.step_hblank_hdma();
        for i in 0..16u8 {
            assert_eq!(
                bus.ppu_read_vram_bank(0, 0x8010 + u16::from(i)),
                i.wrapping_add(0x50)
            );
        }
        assert_eq!(bus.read8(0xFF55), 0xFF);
    }

    #[test]
    fn cgb_hblank_hdma_can_be_cancelled() {
        let mut bus = GbBus::default();
        bus.set_cgb_mode(true);
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        bus.reset();

        bus.write8(0xFF51, 0xC0);
        bus.write8(0xFF52, 0x00);
        bus.write8(0xFF53, 0x00);
        bus.write8(0xFF54, 0x00);
        bus.write8(0xFF55, 0x81);
        bus.step_hblank_hdma();
        assert_eq!(bus.read8(0xFF55) & 0x7F, 0x00);

        bus.write8(0xFF55, 0x00);
        assert_eq!(bus.read8(0xFF55) & 0x80, 0x80);
    }

    #[test]
    fn cgb_key1_toggles_speed_on_stop() {
        let mut bus = GbBus::default();
        bus.set_cgb_mode(true);
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        bus.reset();

        assert_eq!(bus.read8(0xFF4D) & 0x81, 0x00);
        bus.write8(0xFF4D, 0x01);
        assert_eq!(bus.read8(0xFF4D) & 0x81, 0x01);

        bus.handle_stop();
        assert_eq!(bus.read8(0xFF4D) & 0x81, 0x80);
    }

    #[test]
    fn apu_square_channel_generates_nonzero_audio_samples() {
        let mut bus = GbBus::default();
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        bus.reset();

        // Route CH1 to both speakers at max master volume.
        bus.write8(0xFF24, 0x77);
        bus.write8(0xFF25, 0x11);
        bus.write8(0xFF11, 0x80);
        bus.write8(0xFF12, 0xF3);
        bus.write8(0xFF13, 0x40);
        bus.write8(0xFF14, 0xC3); // trigger

        bus.mix_audio_for_cycles(2048);
        let mut samples = Vec::new();
        bus.take_audio_samples_i16_into(&mut samples);
        assert!(!samples.is_empty());
        assert!(samples.iter().any(|&sample| sample != 0));
    }

    #[test]
    fn apu_register_reads_apply_hardware_masks() {
        let mut bus = GbBus::default();
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        bus.reset();

        bus.write8(0xFF13, 0x12);
        bus.write8(0xFF11, 0x80);
        assert_eq!(bus.read8(0xFF13), 0xFF);
        assert_eq!(bus.read8(0xFF11) & 0xC0, 0x80);
        assert_eq!(bus.read8(0xFF11) & 0x3F, 0x3F);
    }
}
