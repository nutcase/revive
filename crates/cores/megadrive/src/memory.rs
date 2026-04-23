use crate::audio::AudioBus;
use crate::cartridge::Cartridge;
use crate::input::{Button, ControllerType, IoBus};
use crate::vdp::{BusDmaRequest, DmaTarget, Vdp, VideoStandard};
use crate::z80::Z80;
use std::collections::VecDeque;

const WORK_RAM_START: u32 = 0xFF0000;
const WORK_RAM_END: u32 = 0xFFFFFF;
const YM2612_START: u32 = 0xA04000;
const YM2612_END: u32 = 0xA04003;
const Z80_RAM_START: u32 = 0xA00000;
const Z80_RAM_END: u32 = 0xA0FFFF;
const IO_VERSION_ADDR: u32 = 0xA10000;
const IO_PORT1_DATA_ADDR: u32 = 0xA10002;
const IO_PORT2_DATA_ADDR: u32 = 0xA10004;
const IO_PORT1_CTRL_ADDR: u32 = 0xA10008;
const IO_PORT2_CTRL_ADDR: u32 = 0xA1000A;
const Z80_BUSREQ_ADDR: u32 = 0xA11100;
const Z80_RESET_ADDR: u32 = 0xA11200;
const TMSS_ADDR_START: u32 = 0xA14000;
const TMSS_ADDR_END: u32 = 0xA14003;
const SRAM_CTRL_ADDR_EVEN: u32 = 0xA130F0;
const SRAM_CTRL_ADDR_ODD: u32 = 0xA130F1;
const VDP_MIRROR_START: u32 = 0xC00000;
const VDP_MIRROR_END: u32 = 0xDFFFFF;
const DMA_BUS_WAIT_CYCLES_PER_WORD: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
enum VdpPort {
    Data,
    Control,
    HvCounter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub enum DmaTraceTarget {
    Vram,
    Cram,
    Vsram,
}

impl From<DmaTarget> for DmaTraceTarget {
    fn from(value: DmaTarget) -> Self {
        match value {
            DmaTarget::Vram => Self::Vram,
            DmaTarget::Cram => Self::Cram,
            DmaTarget::Vsram => Self::Vsram,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct DmaTraceEntry {
    pub target: DmaTraceTarget,
    pub source_addr: u32,
    pub dest_addr: u16,
    pub auto_increment: u16,
    pub words: usize,
    pub first_word: u16,
    pub last_word: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct ActiveBusDma {
    target: DmaTarget,
    source_addr: u32,
    next_source_addr: u32,
    dest_addr: u16,
    auto_increment: u16,
    words_total: usize,
    words_remaining: usize,
    first_word: Option<u16>,
    last_word: u16,
}

impl ActiveBusDma {
    fn from_request(request: BusDmaRequest) -> Self {
        let source_addr = request.source_addr & 0x00FF_FFFE;
        Self {
            target: request.target,
            source_addr,
            next_source_addr: source_addr,
            dest_addr: request.dest_addr,
            auto_increment: request.auto_increment,
            words_total: request.words,
            words_remaining: request.words,
            first_word: None,
            last_word: 0,
        }
    }
}

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct MemoryMap {
    cartridge: Cartridge,
    work_ram: [u8; 0x10000],
    vdp: Vdp,
    vdp_data_write_latch: u16,
    vdp_control_write_latch: u16,
    vdp_data_byte_writes: u64,
    vdp_data_word_writes: u64,
    vdp_control_byte_writes: u64,
    vdp_control_word_writes: u64,
    io: IoBus,
    tmss: [u8; 4],
    z80: Z80,
    audio: AudioBus,
    save_ram_enabled: bool,
    save_ram_write_protect: bool,
    dma_trace: VecDeque<DmaTraceEntry>,
    pending_bus_dma: VecDeque<BusDmaRequest>,
    active_bus_dma: Option<ActiveBusDma>,
    active_bus_dma_cycle_carry: u32,
    dma_wait_cycles: u32,
}

impl MemoryMap {
    #[inline]
    fn mask_address(addr: u32) -> u32 {
        addr & 0x00FF_FFFF
    }

    pub fn new(cartridge: Cartridge) -> Self {
        let region = cartridge.header().region.as_str();
        let io_version = io_version_from_region(region);
        let video_standard = video_standard_from_region(region);
        let mut memory = Self {
            cartridge,
            work_ram: [0; 0x10000],
            vdp: Vdp::with_video_standard(video_standard),
            vdp_data_write_latch: 0,
            vdp_control_write_latch: 0,
            vdp_data_byte_writes: 0,
            vdp_data_word_writes: 0,
            vdp_control_byte_writes: 0,
            vdp_control_word_writes: 0,
            io: IoBus::with_version(io_version),
            tmss: [0; 4],
            z80: Z80::new(),
            audio: AudioBus::new(),
            save_ram_enabled: true,
            save_ram_write_protect: false,
            dma_trace: VecDeque::with_capacity(128),
            pending_bus_dma: VecDeque::with_capacity(8),
            active_bus_dma: None,
            active_bus_dma_cycle_carry: 0,
            dma_wait_cycles: 0,
        };
        memory.apply_cartridge_compat_quirks();
        memory
    }

    pub fn cartridge(&self) -> &Cartridge {
        &self.cartridge
    }

    pub fn vdp(&self) -> &Vdp {
        &self.vdp
    }

    pub fn vdp_mut(&mut self) -> &mut Vdp {
        &mut self.vdp
    }

    pub fn z80(&self) -> &Z80 {
        &self.z80
    }

    pub fn request_z80_interrupt(&mut self) {
        self.z80.request_interrupt();
    }

    pub fn pulse_external_reset(&mut self) {
        // 68000 RESET drives external reset low, then releases it.
        self.z80.write_reset_byte(0x00);
        self.z80.write_reset_byte(0x01);
    }

    pub fn audio(&self) -> &AudioBus {
        &self.audio
    }

    pub fn set_audio_output_sample_rate_hz(&mut self, hz: u32) {
        self.audio.set_output_sample_rate_hz(hz);
    }

    pub fn audio_output_channels(&self) -> u8 {
        self.audio.output_channels()
    }

    pub fn pending_audio_samples(&self) -> usize {
        self.audio.pending_samples()
    }

    pub fn drain_audio_samples(&mut self, max_samples: usize) -> Vec<i16> {
        self.audio.drain_samples(max_samples)
    }

    pub fn set_button_pressed(&mut self, button: Button, pressed: bool) {
        self.io.set_button_pressed(button, pressed);
    }

    pub fn set_button2_pressed(&mut self, button: Button, pressed: bool) {
        self.io.set_button2_pressed(button, pressed);
    }

    pub fn set_controller_type(&mut self, player: u8, controller_type: ControllerType) {
        self.io.set_controller_type(player, controller_type);
    }

    pub fn pending_interrupt_level(&self) -> Option<u8> {
        self.vdp.pending_interrupt_level()
    }

    pub fn acknowledge_interrupt(&mut self, level: u8) {
        self.vdp.acknowledge_interrupt(level);
    }

    pub fn take_dma_wait_cycles(&mut self) -> u32 {
        let cycles = self.dma_wait_cycles;
        self.dma_wait_cycles = 0;
        cycles
    }

    /// Returns true when any DMA operation is active (bus, fill, or copy).
    pub fn dma_active(&self) -> bool {
        self.active_bus_dma.is_some() || !self.pending_bus_dma.is_empty() || self.vdp.dma_busy()
    }

    pub fn step_subsystems(&mut self, cpu_cycles: u32) {
        // Interleave subsystem progress in smaller time slices so Z80 audio
        // writes (especially YM2612 DAC streams) are reflected with better
        // temporal fidelity during long 68k instructions.
        let mut remaining = cpu_cycles;
        while remaining > 0 {
            let slice = remaining.min(4);
            self.z80.step(
                slice,
                &mut self.audio,
                &self.cartridge,
                &mut self.work_ram,
                &mut self.vdp,
                &mut self.io,
            );
            self.io.step(slice);
            self.audio.step(slice);
            remaining -= slice;
        }
    }

    pub fn step_vdp(&mut self, cpu_cycles: u32) -> bool {
        let mut remaining = cpu_cycles;
        let mut frame_ready = false;

        while remaining > 0 {
            if self.active_bus_dma.is_none() {
                self.active_bus_dma = self
                    .pending_bus_dma
                    .pop_front()
                    .map(ActiveBusDma::from_request);
                if self.active_bus_dma.is_some() {
                    self.active_bus_dma_cycle_carry = 0;
                }
            }

            let Some(mut dma) = self.active_bus_dma.take() else {
                self.active_bus_dma_cycle_carry = 0;
                frame_ready |= self.vdp.step(remaining);
                break;
            };

            while remaining > 0 && dma.words_remaining > 0 {
                let needed =
                    DMA_BUS_WAIT_CYCLES_PER_WORD.saturating_sub(self.active_bus_dma_cycle_carry);
                let advance = remaining.min(needed.max(1));
                frame_ready |= self.vdp.step(advance);
                self.active_bus_dma_cycle_carry =
                    self.active_bus_dma_cycle_carry.saturating_add(advance);
                remaining -= advance;

                if self.active_bus_dma_cycle_carry < DMA_BUS_WAIT_CYCLES_PER_WORD {
                    continue;
                }
                self.active_bus_dma_cycle_carry = 0;

                let source = dma.next_source_addr & 0x00FF_FFFE;
                let hi = self.read_u8_mapped(source & 0x00FF_FFFF);
                let lo = self.read_u8_mapped(source.wrapping_add(1) & 0x00FF_FFFF);
                let word = u16::from_be_bytes([hi, lo]);
                if dma.first_word.is_none() {
                    dma.first_word = Some(word);
                }
                dma.last_word = word;
                // Real HW: only the lower 17 bits (REG_DMA_SOURCE_LOW/MID) auto-increment;
                // REG_DMA_SOURCE_HIGH is frozen during transfer, wrapping at 128KB boundary.
                let upper = dma.next_source_addr & !0x0003_FFFE;
                dma.next_source_addr =
                    upper | ((dma.next_source_addr.wrapping_add(2)) & 0x0003_FFFE);
                dma.words_remaining -= 1;

                self.vdp.write_data_port(word);
                self.vdp.refresh_line0_latch_if_active();
            }

            if dma.words_remaining == 0 {
                self.vdp
                    .complete_bus_dma(dma.next_source_addr & 0x00FF_FFFE);
                self.dma_trace.push_back(DmaTraceEntry {
                    target: dma.target.into(),
                    source_addr: dma.source_addr,
                    dest_addr: dma.dest_addr,
                    auto_increment: dma.auto_increment,
                    words: dma.words_total,
                    first_word: dma.first_word.unwrap_or(0),
                    last_word: dma.last_word,
                });
                if self.dma_trace.len() > 128 {
                    self.dma_trace.pop_front();
                }
                self.active_bus_dma = None;
                self.active_bus_dma_cycle_carry = 0;
            } else {
                self.active_bus_dma = Some(dma);
            }
        }

        frame_ready
    }

    pub fn frame_buffer(&self) -> &[u8] {
        self.vdp.frame_buffer()
    }

    pub fn frame_count(&self) -> u64 {
        self.vdp.frame_count()
    }

    pub fn refresh_runtime_after_state_load(&mut self) {
        self.apply_cartridge_compat_quirks();
        self.vdp.refresh_runtime_debug_config_from_env();
    }

    pub fn work_ram(&self) -> &[u8] {
        &self.work_ram
    }

    pub fn work_ram_mut(&mut self) -> &mut [u8] {
        &mut self.work_ram
    }

    pub fn dma_trace(&self) -> Vec<DmaTraceEntry> {
        self.dma_trace.iter().copied().collect()
    }

    pub fn vdp_data_byte_writes(&self) -> u64 {
        self.vdp_data_byte_writes
    }

    pub fn vdp_data_word_writes(&self) -> u64 {
        self.vdp_data_word_writes
    }

    pub fn vdp_control_byte_writes(&self) -> u64 {
        self.vdp_control_byte_writes
    }

    pub fn vdp_control_word_writes(&self) -> u64 {
        self.vdp_control_word_writes
    }

    pub fn read_u8(&mut self, addr: u32) -> u8 {
        let addr = Self::mask_address(addr);
        if decode_vdp_port(addr).is_some() {
            let word = self.read_u16(addr & !1);
            return if addr & 1 == 0 {
                (word >> 8) as u8
            } else {
                word as u8
            };
        }
        self.read_u8_mapped(addr)
    }

    pub fn read_u16(&mut self, addr: u32) -> u16 {
        let addr = Self::mask_address(addr);
        if let Some(port) = decode_vdp_port(addr) {
            return match port {
                VdpPort::Data => self.vdp.read_data_port(),
                VdpPort::Control => self.vdp.read_control_port(),
                VdpPort::HvCounter => self.vdp.read_hv_counter(),
            };
        }

        let hi = self.read_u8_mapped(addr);
        let lo = self.read_u8_mapped(addr.wrapping_add(1));
        u16::from_be_bytes([hi, lo])
    }

    pub fn read_u32(&mut self, addr: u32) -> u32 {
        let addr = Self::mask_address(addr);
        let hi = self.read_u16(addr) as u32;
        let lo = self.read_u16(addr.wrapping_add(2)) as u32;
        (hi << 16) | lo
    }

    pub fn write_u8(&mut self, addr: u32, value: u8) {
        let addr = Self::mask_address(addr);
        if let Some(port) = decode_vdp_port(addr) {
            let immediate_byte_commit = crate::debug_flags::vdp_byte_immediate();
            let low_byte_write = (addr & 1) != 0;
            let next = match port {
                VdpPort::Data => {
                    let current = self.vdp_data_write_latch;
                    if addr & 1 == 0 {
                        ((value as u16) << 8) | (current & 0x00FF)
                    } else {
                        (current & 0xFF00) | value as u16
                    }
                }
                VdpPort::Control => {
                    let current = self.vdp_control_write_latch;
                    if addr & 1 == 0 {
                        ((value as u16) << 8) | (current & 0x00FF)
                    } else {
                        (current & 0xFF00) | value as u16
                    }
                }
                VdpPort::HvCounter => 0,
            };
            match port {
                VdpPort::Data => {
                    self.vdp_data_byte_writes = self.vdp_data_byte_writes.saturating_add(1);
                    self.vdp_data_write_latch = next;
                    if immediate_byte_commit || low_byte_write {
                        let wait = self.vdp.fifo_wait_cycles();
                        self.dma_wait_cycles = self.dma_wait_cycles.saturating_add(wait);
                        self.vdp.write_data_port(next);
                    }
                }
                VdpPort::Control => {
                    self.vdp_control_byte_writes = self.vdp_control_byte_writes.saturating_add(1);
                    self.vdp_control_write_latch = next;
                    if immediate_byte_commit || low_byte_write {
                        self.vdp.write_control_port(next);
                        self.enqueue_pending_vdp_dma();
                    }
                }
                VdpPort::HvCounter => {}
            }
            return;
        }
        self.write_u8_mapped(addr, value);
    }

    pub fn write_u16(&mut self, addr: u32, value: u16) {
        let addr = Self::mask_address(addr);
        if let Some(port) = decode_vdp_port(addr) {
            match port {
                VdpPort::Data => {
                    self.vdp_data_word_writes = self.vdp_data_word_writes.saturating_add(1);
                    self.vdp_data_write_latch = value;
                    // Stall CPU if FIFO is full
                    let wait = self.vdp.fifo_wait_cycles();
                    self.dma_wait_cycles = self.dma_wait_cycles.saturating_add(wait);
                    self.vdp.write_data_port(value);
                }
                VdpPort::Control => {
                    self.vdp_control_word_writes = self.vdp_control_word_writes.saturating_add(1);
                    self.vdp_control_write_latch = value;
                    self.vdp.write_control_port(value);
                    self.enqueue_pending_vdp_dma();
                }
                VdpPort::HvCounter => {}
            }
            return;
        }

        let [hi, lo] = value.to_be_bytes();
        self.write_u8_mapped(addr, hi);
        self.write_u8_mapped(addr.wrapping_add(1), lo);
    }

    pub fn write_u32(&mut self, addr: u32, value: u32) {
        let addr = Self::mask_address(addr);
        let [b0, b1, b2, b3] = value.to_be_bytes();
        self.write_u16(addr, u16::from_be_bytes([b0, b1]));
        self.write_u16(addr.wrapping_add(2), u16::from_be_bytes([b2, b3]));
    }

    fn read_u8_mapped(&mut self, addr: u32) -> u8 {
        match addr {
            SRAM_CTRL_ADDR_EVEN => 0x00,
            SRAM_CTRL_ADDR_ODD => {
                (self.save_ram_enabled as u8) | ((self.save_ram_write_protect as u8) << 1)
            }
            // Sega mapper bank register reads (0xA130F2..0xA130FF, even bytes)
            0xA130F2..=0xA130FF if (addr & 1) == 0 => {
                let reg_index = ((addr - 0xA130F0) >> 1) as usize;
                self.cartridge.read_bank_register(reg_index)
            }
            0x000000..=0x3FFFFF => {
                // EEPROM mapped region
                if self.cartridge.eeprom_mapped(addr) {
                    return self.cartridge.read_eeprom(addr);
                }
                if self.save_ram_enabled
                    && let Some(value) = self.cartridge.read_save_ram_u8(addr)
                {
                    value
                } else {
                    self.cartridge.read_u8(addr)
                }
            }
            WORK_RAM_START..=WORK_RAM_END => self.work_ram[(addr - WORK_RAM_START) as usize],
            IO_VERSION_ADDR => self.io.read_version(),
            x if x == IO_VERSION_ADDR + 1 => self.io.read_version(),
            x if x == IO_PORT1_DATA_ADDR || x == IO_PORT1_DATA_ADDR + 1 => {
                self.io.read_port1_data()
            }
            x if x == IO_PORT2_DATA_ADDR || x == IO_PORT2_DATA_ADDR + 1 => {
                self.io.read_port2_data()
            }
            x if x == IO_PORT1_CTRL_ADDR || x == IO_PORT1_CTRL_ADDR + 1 => {
                self.io.read_port1_ctrl()
            }
            x if x == IO_PORT2_CTRL_ADDR || x == IO_PORT2_CTRL_ADDR + 1 => {
                self.io.read_port2_ctrl()
            }
            Z80_BUSREQ_ADDR => self.z80.read_busreq_byte(),
            x if x == Z80_BUSREQ_ADDR + 1 => 0x00,
            Z80_RESET_ADDR => self.z80.read_reset_byte(),
            x if x == Z80_RESET_ADDR + 1 => 0x00,
            TMSS_ADDR_START..=TMSS_ADDR_END => self.tmss[(addr - TMSS_ADDR_START) as usize],
            YM2612_START..=YM2612_END => self.audio.read_ym2612((addr - YM2612_START) as u8),
            Z80_RAM_START..=Z80_RAM_END => {
                if self.z80.m68k_can_access_ram() {
                    self.z80.read_ram_u8((addr - Z80_RAM_START) as u16)
                } else {
                    0xFF
                }
            }
            _ => 0xFF,
        }
    }

    fn write_u8_mapped(&mut self, addr: u32, value: u8) {
        match addr {
            SRAM_CTRL_ADDR_EVEN => {}
            SRAM_CTRL_ADDR_ODD => {
                self.save_ram_enabled = (value & 0x01) != 0;
                self.save_ram_write_protect = (value & 0x02) != 0;
            }
            // Sega mapper bank register writes (0xA130F2..0xA130FF, even bytes)
            0xA130F2..=0xA130FF if (addr & 1) == 0 => {
                let reg_index = ((addr - 0xA130F0) >> 1) as usize;
                self.cartridge.write_bank_register(reg_index, value);
            }
            0x000000..=0x3FFFFF => {
                // EEPROM: only even byte triggers I2C (odd byte is silently absorbed)
                if self.cartridge.eeprom_mapped(addr) {
                    if self.cartridge.eeprom_write_triggers(addr) {
                        self.cartridge.write_eeprom(addr, value);
                    }
                    return;
                }
                if self.save_ram_enabled
                    && !self.save_ram_write_protect
                    && self.cartridge.write_save_ram_u8(addr, value)
                {
                    return;
                }
            }
            WORK_RAM_START..=WORK_RAM_END => {
                self.work_ram[(addr - WORK_RAM_START) as usize] = value;
            }
            x if x == IO_PORT1_DATA_ADDR || x == IO_PORT1_DATA_ADDR + 1 => {
                self.io.write_port1_data(value);
            }
            x if x == IO_PORT2_DATA_ADDR || x == IO_PORT2_DATA_ADDR + 1 => {
                self.io.write_port2_data(value);
            }
            x if x == IO_PORT1_CTRL_ADDR || x == IO_PORT1_CTRL_ADDR + 1 => {
                self.io.write_port1_ctrl(value);
            }
            x if x == IO_PORT2_CTRL_ADDR || x == IO_PORT2_CTRL_ADDR + 1 => {
                self.io.write_port2_ctrl(value);
            }
            Z80_BUSREQ_ADDR => self.z80.write_busreq_byte(value),
            Z80_RESET_ADDR => self.z80.write_reset_byte(value),
            TMSS_ADDR_START..=TMSS_ADDR_END => {
                self.tmss[(addr - TMSS_ADDR_START) as usize] = value;
            }
            YM2612_START..=YM2612_END => {
                self.audio.write_ym2612((addr - YM2612_START) as u8, value)
            }
            Z80_RAM_START..=Z80_RAM_END => {
                if self.z80.m68k_can_access_ram() {
                    self.z80.write_ram_u8((addr - Z80_RAM_START) as u16, value);
                }
            }
            x if is_psg_write_addr(x) => self.audio.write_psg(value),
            _ => {}
        }
    }

    fn enqueue_pending_vdp_dma(&mut self) {
        while let Some(request) = self.vdp.take_bus_dma_request() {
            self.dma_wait_cycles = self.dma_wait_cycles.saturating_add(
                (request.words as u32).saturating_mul(DMA_BUS_WAIT_CYCLES_PER_WORD),
            );
            self.pending_bus_dma.push_back(BusDmaRequest {
                source_addr: request.source_addr & 0x00FF_FFFE,
                dest_addr: request.dest_addr,
                auto_increment: request.auto_increment,
                words: request.words,
                target: request.target,
            });
        }
    }

    fn apply_cartridge_compat_quirks(&mut self) {
        let _ = &self.cartridge;
    }
}

#[cfg(test)]
fn comix_zone_compat_quirks_enabled(cartridge: &Cartridge) -> bool {
    let header = cartridge.header();
    if header.product_code.contains("G-4132") {
        return true;
    }
    let domestic = header.domestic_title.to_ascii_uppercase();
    let overseas = header.overseas_title.to_ascii_uppercase();
    domestic.contains("COMIX ZONE") || overseas.contains("COMIX ZONE")
}

fn io_version_from_region(region: &str) -> u8 {
    let upper = region.trim().to_ascii_uppercase();
    if upper.contains('J') {
        return 0x20; // Japan NTSC
    }
    if upper.contains('U') {
        return 0xA0; // Overseas NTSC
    }
    if upper.contains('E') {
        return 0xE0; // Overseas PAL
    }

    let mut numeric_mask = 0u8;
    for ch in upper.chars() {
        if let Some(digit) = ch.to_digit(16) {
            numeric_mask |= digit as u8;
        }
    }

    if (numeric_mask & 0x01) != 0 {
        0x20
    } else if (numeric_mask & 0x04) != 0 {
        0xA0
    } else if (numeric_mask & 0x08) != 0 {
        0xE0
    } else {
        0x20
    }
}

fn video_standard_from_region(region: &str) -> VideoStandard {
    let upper = region.trim().to_ascii_uppercase();
    if upper.contains('J') || upper.contains('U') {
        return VideoStandard::Ntsc;
    }
    if upper.contains('E') {
        return VideoStandard::Pal;
    }

    let mut numeric_mask = 0u8;
    for ch in upper.chars() {
        if let Some(digit) = ch.to_digit(16) {
            numeric_mask |= digit as u8;
        }
    }

    if (numeric_mask & 0x05) != 0 {
        VideoStandard::Ntsc
    } else if (numeric_mask & 0x08) != 0 {
        VideoStandard::Pal
    } else {
        VideoStandard::Ntsc
    }
}

fn decode_vdp_port(addr: u32) -> Option<VdpPort> {
    let local = decode_vdp_local_addr(addr)?;
    let aligned = local & !1;
    match aligned {
        0xC00000 | 0xC00002 => Some(VdpPort::Data),
        0xC00004 | 0xC00006 => Some(VdpPort::Control),
        0xC00008 | 0xC0000A => Some(VdpPort::HvCounter),
        _ => None,
    }
}

fn decode_vdp_local_addr(addr: u32) -> Option<u32> {
    if (VDP_MIRROR_START..=VDP_MIRROR_END).contains(&addr) {
        Some(0xC00000 | (addr & 0x1F))
    } else {
        None
    }
}

fn is_psg_write_addr(addr: u32) -> bool {
    let Some(local) = decode_vdp_local_addr(addr) else {
        return false;
    };
    matches!(local, 0xC00011 | 0xC00013 | 0xC00015 | 0xC00017)
}

#[cfg(test)]
mod tests {
    use crate::cartridge::Cartridge;
    use crate::input::Button;
    use crate::memory::{
        DmaTraceTarget, MemoryMap, comix_zone_compat_quirks_enabled, io_version_from_region,
        video_standard_from_region,
    };
    use crate::vdp::VideoStandard;

    #[test]
    fn maps_work_ram_reads_and_writes() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        memory.write_u8(0xFF0000, 0x12);
        memory.write_u16(0xFF0002, 0xABCD);

        assert_eq!(memory.read_u8(0xFF0000), 0x12);
        assert_eq!(memory.read_u16(0xFF0002), 0xABCD);
    }

    #[test]
    fn comix_zone_compat_quirk_detects_product_code() {
        let mut rom = vec![0; 0x400];
        rom[0x182..0x18E].copy_from_slice(b" G-4132  -00");
        let cart = Cartridge::from_bytes(rom).expect("valid cart");
        assert!(comix_zone_compat_quirks_enabled(&cart));
    }

    #[test]
    fn comix_zone_compat_quirk_detects_domestic_title() {
        let mut rom = vec![0; 0x400];
        let mut title = [b' '; 48];
        title[..10].copy_from_slice(b"COMIX ZONE");
        rom[0x120..0x150].copy_from_slice(&title);
        let cart = Cartridge::from_bytes(rom).expect("valid cart");
        assert!(comix_zone_compat_quirks_enabled(&cart));
    }

    #[test]
    fn routes_vdp_ports_for_vram_write() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        memory.write_u16(0xC00004, 0x4000);
        memory.write_u16(0xC00004, 0x0000);
        memory.write_u16(0xC00000, 0xABCD);

        assert_eq!(memory.vdp().read_vram_u8(0), 0xAB);
        assert_eq!(memory.vdp().read_vram_u8(1), 0xCD);
    }

    #[test]
    fn vdp_data_port_byte_writes_commit_only_on_second_byte() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        memory.write_u16(0xC00004, 0x4000);
        memory.write_u16(0xC00004, 0x0000);
        memory.write_u16(0xC00000, 0xA1B2);

        memory.write_u16(0xC00004, 0x4000);
        memory.write_u16(0xC00004, 0x0000);
        memory.write_u8(0xC00000, 0x12);
        assert_eq!(memory.vdp().read_vram_u8(0), 0xA1);
        assert_eq!(memory.vdp().read_vram_u8(1), 0xB2);

        memory.write_u8(0xC00001, 0x34);
        assert_eq!(memory.vdp().read_vram_u8(0), 0x12);
        assert_eq!(memory.vdp().read_vram_u8(1), 0x34);
    }

    #[test]
    fn vdp_control_port_byte_writes_commit_only_on_second_byte() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        // Baseline write at VRAM address 0x0020.
        memory.write_u16(0xC00004, 0x4020);
        memory.write_u16(0xC00004, 0x0000);
        memory.write_u16(0xC00000, 0xAAAA);
        assert_eq!(memory.vdp().read_vram_u8(0x20), 0xAA);
        assert_eq!(memory.vdp().read_vram_u8(0x21), 0xAA);

        // First byte only should not commit a new command.
        memory.write_u8(0xC00004, 0x40);
        memory.write_u16(0xC00000, 0xBBBB);
        assert_eq!(memory.vdp().read_vram_u8(0x20), 0xAA);
        assert_eq!(memory.vdp().read_vram_u8(0x21), 0xAA);

        // Complete command with byte pairs and write new value at 0x0020.
        memory.write_u8(0xC00005, 0x20);
        memory.write_u8(0xC00004, 0x00);
        memory.write_u8(0xC00005, 0x00);
        memory.write_u16(0xC00000, 0x1234);
        assert_eq!(memory.vdp().read_vram_u8(0x20), 0x12);
        assert_eq!(memory.vdp().read_vram_u8(0x21), 0x34);
    }

    #[test]
    fn routes_vdp_control_port_long_writes_as_two_control_words() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        memory.write_u32(0xC00004, 0x4000_0000);
        memory.write_u16(0xC00000, 0xABCD);

        assert_eq!(memory.vdp().read_vram_u8(0x0000), 0xAB);
        assert_eq!(memory.vdp().read_vram_u8(0x0001), 0xCD);
    }

    #[test]
    fn routes_vdp_hv_counter_port() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        let before = memory.read_u16(0xC00008);
        memory.step_vdp(1_000);
        let after = memory.read_u16(0xC00008);

        assert_ne!(before, after);
        assert_eq!(memory.read_u16(0xC00008), memory.vdp().read_hv_counter());
    }

    #[test]
    fn routes_controller_ports() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        assert_eq!(memory.read_u8(0xA10003), 0x7F);

        memory.set_button_pressed(Button::A, true);
        memory.set_button_pressed(Button::Start, true);
        memory.write_u8(0xA10003, 0x00); // TH low
        assert_eq!(memory.read_u8(0xA10003), 0x03);
    }

    #[test]
    fn io_version_follows_rom_region_letters() {
        let mut rom = vec![0; 0x400];
        rom[0x1F0..0x1F3].copy_from_slice(b"U  ");
        let cart = Cartridge::from_bytes(rom).expect("valid cart");
        let mut memory = MemoryMap::new(cart);
        assert_eq!(memory.read_u8(0xA10000), 0xA0);

        let mut rom = vec![0; 0x400];
        rom[0x1F0..0x1F3].copy_from_slice(b"E  ");
        let cart = Cartridge::from_bytes(rom).expect("valid cart");
        let mut memory = MemoryMap::new(cart);
        assert_eq!(memory.read_u8(0xA10000), 0xE0);
    }

    #[test]
    fn io_version_follows_numeric_region_mask() {
        assert_eq!(io_version_from_region("4"), 0xA0);
        assert_eq!(io_version_from_region("8"), 0xE0);
        assert_eq!(io_version_from_region("1"), 0x20);
        assert_eq!(io_version_from_region("F"), 0x20);
    }

    #[test]
    fn video_standard_follows_rom_region() {
        assert_eq!(video_standard_from_region("E"), VideoStandard::Pal);
        assert_eq!(video_standard_from_region("8"), VideoStandard::Pal);
        assert_eq!(video_standard_from_region("J"), VideoStandard::Ntsc);
        assert_eq!(video_standard_from_region("U"), VideoStandard::Ntsc);
        assert_eq!(video_standard_from_region("JUE"), VideoStandard::Ntsc);
        assert_eq!(video_standard_from_region("C"), VideoStandard::Ntsc);
    }

    #[test]
    fn memory_map_constructs_pal_vdp_for_pal_region_rom() {
        let mut rom = vec![0; 0x400];
        rom[0x1F0..0x1F1].copy_from_slice(b"E");
        let cart = Cartridge::from_bytes(rom).expect("valid cart");
        let memory = MemoryMap::new(cart);
        assert_eq!(memory.vdp().video_standard(), VideoStandard::Pal);
        assert_eq!(memory.vdp().total_lines(), 313);
    }

    #[test]
    fn routes_second_controller_ports() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        assert_eq!(memory.read_u8(0xA10005), 0x7F);

        memory.set_button2_pressed(Button::Left, true);
        memory.set_button2_pressed(Button::C, true);
        assert_eq!(memory.read_u8(0xA10005), 0x5B);

        memory.set_button2_pressed(Button::A, true);
        memory.set_button2_pressed(Button::Start, true);
        memory.write_u8(0xA10005, 0x00); // TH low
        assert_eq!(memory.read_u8(0xA10005), 0x03);
    }

    #[test]
    fn routes_z80_bus_control_ports() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        memory.write_u16(0xA11100, 0x0100);
        assert_eq!(memory.read_u16(0xA11100), 0x0100);
        memory.step_subsystems(16);
        assert_eq!(memory.read_u16(0xA11100), 0x0000);

        memory.write_u16(0xA11200, 0x0100);
        assert_eq!(memory.read_u16(0xA11200), 0x0100);
    }

    #[test]
    fn pulse_external_reset_restarts_z80_core() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        memory.write_u16(0xA11200, 0x0100); // release reset
        memory.write_u16(0xA11100, 0x0000); // ensure bus is owned by Z80
        memory.step_subsystems(40);
        assert!(memory.z80().pc() > 0);

        memory.pulse_external_reset();
        assert_eq!(memory.z80().read_reset_byte(), 0x01);
        assert_eq!(memory.z80().pc(), 0);
    }

    #[test]
    fn routes_audio_ports() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        memory.write_u8(0xA04000, 0x22);
        memory.write_u8(0xA04001, 0x0F);
        assert_eq!(memory.audio().ym2612().register(0, 0x22), 0x0F);

        memory.write_u8(0xC00011, 0x9F);
        assert_eq!(memory.audio().psg().last_data(), 0x9F);
    }

    #[test]
    fn routes_psg_mirror_addresses() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        memory.write_u8(0xC00013, 0x91);
        assert_eq!(memory.audio().psg().last_data(), 0x91);
        memory.write_u8(0xD00011, 0x92);
        assert_eq!(memory.audio().psg().last_data(), 0x92);
        memory.write_u8(0xD00017, 0x93);
        assert_eq!(memory.audio().psg().last_data(), 0x93);
    }

    #[test]
    fn routes_vdp_port_mirror_addresses() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        // Register #15 (auto-increment) via mirrored control port.
        memory.write_u16(0xD00004, 0x8F02);
        assert_eq!(memory.vdp().register(15), 0x02);

        // Data port write via mirrored data address.
        memory.write_u16(0xD00004, 0x4000);
        memory.write_u16(0xD00004, 0x0000);
        memory.write_u16(0xD00000, 0xABCD);
        assert_eq!(memory.vdp().read_vram_u8(0), 0xAB);
        assert_eq!(memory.vdp().read_vram_u8(1), 0xCD);
    }

    #[test]
    fn routes_tmss_register_reads_and_writes() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        memory.write_u32(0xA14000, 0x5345_4741); // "SEGA"
        assert_eq!(memory.read_u8(0xA14000), b'S');
        assert_eq!(memory.read_u8(0xA14001), b'E');
        assert_eq!(memory.read_u16(0xA14002), u16::from_be_bytes([b'G', b'A']));

        memory.write_u16(0xA14000, 0x4D44); // "MD"
        assert_eq!(memory.read_u16(0xA14000), 0x4D44);
    }

    #[test]
    fn maps_cartridge_save_ram_and_control_register() {
        let mut rom = vec![0u8; 0x400];
        rom[0x201] = 0x7A; // ROM value used when save RAM is disabled.
        rom[0x1B0..0x1B2].copy_from_slice(b"RA");
        rom[0x1B4..0x1B8].copy_from_slice(&0x0000_0201u32.to_be_bytes());
        rom[0x1B8..0x1BC].copy_from_slice(&0x0000_020Fu32.to_be_bytes());
        let cart = Cartridge::from_bytes(rom).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        // Save RAM is enabled by default and starts erased.
        assert_eq!(memory.read_u8(0x000201), 0xFF);
        memory.write_u8(0x000201, 0x12);
        assert_eq!(memory.read_u8(0x000201), 0x12);
        // Odd-lane SRAM returns open bus on even addresses in-range.
        assert_eq!(memory.read_u8(0x000202), 0xFF);

        // Disabling save RAM falls back to ROM reads.
        memory.write_u16(0xA130F0, 0x0000);
        assert_eq!(memory.read_u8(0x000201), 0x7A);

        // Re-enable save RAM and verify persisted value remains.
        memory.write_u16(0xA130F0, 0x0001);
        assert_eq!(memory.read_u8(0x000201), 0x12);

        // Write-protect blocks writes.
        memory.write_u16(0xA130F0, 0x0003);
        assert_eq!(memory.read_u16(0xA130F0), 0x0003);
        memory.write_u8(0x000201, 0x34);
        assert_eq!(memory.read_u8(0x000201), 0x12);
    }

    #[test]
    fn eeprom_word_access_toggles_i2c_once() {
        // Build a ROM that triggers EEPROM detection (NBA Jam product code).
        let mut rom = vec![0u8; 0x400];
        rom[0x180..0x18E].copy_from_slice(b"T-81326 -00\x00\x00\x00");
        let cart = Cartridge::from_bytes(rom).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        let scl: u16 = 0x0002;
        let sda: u16 = 0x0001;
        let addr: u32 = 0x200000;

        // Helper: write a word to EEPROM address (only even byte should trigger I2C)
        let i2c_word_write = |m: &mut MemoryMap, val: u16| {
            m.write_u16(addr, val);
        };

        // I2C START: SDA falls while SCL high
        i2c_word_write(&mut memory, (scl | sda) as u16);
        i2c_word_write(&mut memory, scl as u16); // SDA low, SCL high = START

        // Send device address 0xA0 (write) via word writes — each word write
        // should toggle I2C exactly once, not twice.
        let device_byte: u8 = 0xA0;
        for i in (0..8).rev() {
            let sda_val = if (device_byte >> i) & 1 != 0 { sda } else { 0 };
            i2c_word_write(&mut memory, sda_val as u16); // SCL low
            i2c_word_write(&mut memory, (sda_val | scl) as u16); // SCL high
            i2c_word_write(&mut memory, sda_val as u16); // SCL low
        }
        // ACK clock
        i2c_word_write(&mut memory, 0);
        i2c_word_write(&mut memory, scl as u16);
        i2c_word_write(&mut memory, 0);

        // Send word address 0x00
        for _ in 0..8 {
            i2c_word_write(&mut memory, 0);
            i2c_word_write(&mut memory, scl as u16);
            i2c_word_write(&mut memory, 0);
        }
        i2c_word_write(&mut memory, 0);
        i2c_word_write(&mut memory, scl as u16);
        i2c_word_write(&mut memory, 0);

        // Send data 0x55
        let data_byte: u8 = 0x55;
        for i in (0..8).rev() {
            let sda_val = if (data_byte >> i) & 1 != 0 { sda } else { 0 };
            i2c_word_write(&mut memory, sda_val as u16);
            i2c_word_write(&mut memory, (sda_val | scl) as u16);
            i2c_word_write(&mut memory, sda_val as u16);
        }
        i2c_word_write(&mut memory, 0);
        i2c_word_write(&mut memory, scl as u16);
        i2c_word_write(&mut memory, 0);

        // STOP
        i2c_word_write(&mut memory, 0);
        i2c_word_write(&mut memory, scl as u16);
        i2c_word_write(&mut memory, (scl | sda) as u16);

        // Read back via sequential read: START, dev write, word addr, re-START, dev read
        i2c_word_write(&mut memory, (scl | sda) as u16);
        i2c_word_write(&mut memory, scl as u16); // START

        // Device write 0xA0
        for i in (0..8).rev() {
            let sda_val = if (0xA0u8 >> i) & 1 != 0 { sda } else { 0 };
            i2c_word_write(&mut memory, sda_val as u16);
            i2c_word_write(&mut memory, (sda_val | scl) as u16);
            i2c_word_write(&mut memory, sda_val as u16);
        }
        i2c_word_write(&mut memory, 0);
        i2c_word_write(&mut memory, scl as u16);
        i2c_word_write(&mut memory, 0);

        // Word addr 0x00
        for _ in 0..8 {
            i2c_word_write(&mut memory, 0);
            i2c_word_write(&mut memory, scl as u16);
            i2c_word_write(&mut memory, 0);
        }
        i2c_word_write(&mut memory, 0);
        i2c_word_write(&mut memory, scl as u16);
        i2c_word_write(&mut memory, 0);

        // Re-START
        i2c_word_write(&mut memory, sda as u16);
        i2c_word_write(&mut memory, (scl | sda) as u16);
        i2c_word_write(&mut memory, scl as u16);

        // Device read 0xA1
        for i in (0..8).rev() {
            let sda_val = if (0xA1u8 >> i) & 1 != 0 { sda } else { 0 };
            i2c_word_write(&mut memory, sda_val as u16);
            i2c_word_write(&mut memory, (sda_val | scl) as u16);
            i2c_word_write(&mut memory, sda_val as u16);
        }
        i2c_word_write(&mut memory, 0);
        i2c_word_write(&mut memory, scl as u16);
        i2c_word_write(&mut memory, 0);

        // Read 8 bits via word reads
        let mut read_byte = 0u8;
        for _ in 0..8 {
            i2c_word_write(&mut memory, 0);
            i2c_word_write(&mut memory, scl as u16);
            let word = memory.read_u16(addr);
            let bit = word & (sda as u16);
            read_byte = (read_byte << 1) | if bit != 0 { 1 } else { 0 };
        }

        assert_eq!(
            read_byte, 0x55,
            "EEPROM word-access read should return 0x55"
        );
    }

    #[test]
    fn sega_mapper_slot0_is_fixed() {
        let mut rom = vec![0u8; 0x200000];
        rom[0x000000] = 0xAA;
        rom[0x080000] = 0xBB;

        let cart = Cartridge::from_bytes(rom).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        // Write to mapper reg0 (0xA130F2) should be ignored — slot 0 is fixed
        memory.write_u16(0xA130F2, 0x0001);
        assert_eq!(memory.read_u8(0x000000), 0xAA); // still bank 0
    }

    #[test]
    fn exposes_generated_audio_samples() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        memory.write_u8(0xC00011, 0x90);
        memory.step_subsystems(2_000);

        assert!(memory.pending_audio_samples() > 0);
        let samples = memory.drain_audio_samples(64);
        assert!(!samples.is_empty());
        assert!(samples.iter().any(|&s| s != 0));
    }

    #[test]
    fn z80_ram_access_requires_bus_request() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        memory.write_u8(0xA00010, 0x12); // ignored while bus not requested
        assert_eq!(memory.read_u8(0xA00010), 0xFF);

        memory.write_u16(0xA11100, 0x0100); // request Z80 bus
        memory.write_u8(0xA00010, 0x22); // still ignored before grant
        assert_eq!(memory.read_u8(0xA00010), 0xFF);
        memory.step_subsystems(16);
        memory.write_u8(0xA00010, 0x34);
        assert_eq!(memory.read_u8(0xA00010), 0x34);
    }

    #[test]
    fn z80_ram_is_mirrored_over_8kb_window() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);
        memory.write_u16(0xA11100, 0x0100); // request Z80 bus
        memory.step_subsystems(16);

        memory.write_u8(0xA00001, 0x56);
        assert_eq!(memory.read_u8(0xA02001), 0x56);
    }

    #[test]
    fn runs_vdp_dma_from_68k_bus_into_vram() {
        let mut rom = vec![0; 0x400];
        rom[0x200] = 0xAA;
        rom[0x201] = 0xBB;
        rom[0x202] = 0xCC;
        rom[0x203] = 0xDD;

        let cart = Cartridge::from_bytes(rom).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        // Enable VDP DMA (reg1 bit4) and set increment=2.
        memory.write_u16(0xC00004, 0x8150);
        memory.write_u16(0xC00004, 0x8F02);
        // DMA length = 2 words.
        memory.write_u16(0xC00004, 0x9302);
        memory.write_u16(0xC00004, 0x9400);
        // DMA source (word address): 0x000200 >> 1 = 0x000100.
        memory.write_u16(0xC00004, 0x9500);
        memory.write_u16(0xC00004, 0x9601);
        memory.write_u16(0xC00004, 0x9700); // mode 00: 68k->VDP

        // VRAM write DMA command @ 0x0000.
        memory.write_u16(0xC00004, 0x4000);
        memory.write_u16(0xC00004, 0x0080);

        let dma_cycles = memory.take_dma_wait_cycles();
        assert_eq!(dma_cycles, 4);
        memory.step_vdp(dma_cycles);

        assert_eq!(memory.vdp().read_vram_u8(0x0000), 0xAA);
        assert_eq!(memory.vdp().read_vram_u8(0x0001), 0xBB);
        assert_eq!(memory.vdp().read_vram_u8(0x0002), 0xCC);
        assert_eq!(memory.vdp().read_vram_u8(0x0003), 0xDD);
        assert_eq!(memory.take_dma_wait_cycles(), 0);
        let trace = memory.dma_trace();
        let last = trace.last().expect("dma trace entry");
        assert_eq!(last.target, DmaTraceTarget::Vram);
        assert_eq!(last.source_addr, 0x00000200);
        assert_eq!(last.dest_addr, 0x0000);
        assert_eq!(last.auto_increment, 2);
        assert_eq!(last.words, 2);
        assert_eq!(last.first_word, 0xAABB);
        assert_eq!(last.last_word, 0xCCDD);
    }

    #[test]
    fn bus_dma_updates_line0_latch_when_triggered_at_frame_start() {
        let mut rom = vec![0; 0x400];
        rom[0x200] = 0xAA;
        rom[0x201] = 0xBB;
        rom[0x202] = 0xCC;
        rom[0x203] = 0xDD;

        let cart = Cartridge::from_bytes(rom).expect("valid cart");
        let mut memory = MemoryMap::new(cart);
        memory.vdp_mut().set_line_vram_latch_enabled_for_debug(true);

        memory.write_u16(0xC00004, 0x8150);
        memory.write_u16(0xC00004, 0x8F02);
        memory.write_u16(0xC00004, 0x9302);
        memory.write_u16(0xC00004, 0x9400);
        memory.write_u16(0xC00004, 0x9500);
        memory.write_u16(0xC00004, 0x9601);
        memory.write_u16(0xC00004, 0x9700);
        memory.write_u16(0xC00004, 0x4000);
        memory.write_u16(0xC00004, 0x0080);

        memory.step_vdp(4);

        assert_eq!(memory.vdp().read_vram_u8(0x0000), 0xAA);
        assert_eq!(memory.vdp().read_vram_u8(0x0001), 0xBB);
        assert_eq!(memory.vdp().read_vram_u8(0x0002), 0xCC);
        assert_eq!(memory.vdp().read_vram_u8(0x0003), 0xDD);
        assert_eq!(memory.vdp().line_vram_u8(0, 0x0000), 0xAA);
        assert_eq!(memory.vdp().line_vram_u8(0, 0x0001), 0xBB);
        assert_eq!(memory.vdp().line_vram_u8(0, 0x0002), 0xCC);
        assert_eq!(memory.vdp().line_vram_u8(0, 0x0003), 0xDD);
    }

    #[test]
    fn bus_dma_respects_zero_auto_increment() {
        let mut rom = vec![0; 0x400];
        rom[0x200] = 0xAA;
        rom[0x201] = 0xBB;
        rom[0x202] = 0xCC;
        rom[0x203] = 0xDD;

        let cart = Cartridge::from_bytes(rom).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        // Enable VDP DMA (reg1 bit4) and set increment=0.
        memory.write_u16(0xC00004, 0x8150);
        memory.write_u16(0xC00004, 0x8F00);
        // DMA length = 2 words.
        memory.write_u16(0xC00004, 0x9302);
        memory.write_u16(0xC00004, 0x9400);
        // DMA source (word address): 0x000200 >> 1 = 0x000100.
        memory.write_u16(0xC00004, 0x9500);
        memory.write_u16(0xC00004, 0x9601);
        memory.write_u16(0xC00004, 0x9700); // mode 00: 68k->VDP

        // VRAM write DMA command @ 0x0000.
        memory.write_u16(0xC00004, 0x4000);
        memory.write_u16(0xC00004, 0x0080);

        let dma_cycles = memory.take_dma_wait_cycles();
        assert_eq!(dma_cycles, 4);
        memory.step_vdp(dma_cycles);

        // increment=0 keeps destination fixed, so only the latest word remains.
        assert_eq!(memory.vdp().read_vram_u8(0x0000), 0xCC);
        assert_eq!(memory.vdp().read_vram_u8(0x0001), 0xDD);
        assert_eq!(memory.vdp().read_vram_u8(0x0002), 0x00);
        assert_eq!(memory.vdp().read_vram_u8(0x0003), 0x00);

        let trace = memory.dma_trace();
        let last = trace.last().expect("dma trace entry");
        assert_eq!(last.auto_increment, 0);
        assert_eq!(last.first_word, 0xAABB);
        assert_eq!(last.last_word, 0xCCDD);
    }

    #[test]
    fn bus_dma_progresses_as_vdp_cycles_advance() {
        let mut rom = vec![0; 0x400];
        rom[0x200] = 0xAA;
        rom[0x201] = 0xBB;
        rom[0x202] = 0xCC;
        rom[0x203] = 0xDD;

        let cart = Cartridge::from_bytes(rom).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        memory.write_u16(0xC00004, 0x8150);
        memory.write_u16(0xC00004, 0x8F02);
        memory.write_u16(0xC00004, 0x9302);
        memory.write_u16(0xC00004, 0x9400);
        memory.write_u16(0xC00004, 0x9500);
        memory.write_u16(0xC00004, 0x9601);
        memory.write_u16(0xC00004, 0x9700);
        memory.write_u16(0xC00004, 0x4000);
        memory.write_u16(0xC00004, 0x0080);

        // 2 cycles transfer exactly one word.
        memory.step_vdp(2);
        assert_eq!(memory.vdp().read_vram_u8(0x0000), 0xAA);
        assert_eq!(memory.vdp().read_vram_u8(0x0001), 0xBB);
        assert_eq!(memory.vdp().read_vram_u8(0x0002), 0x00);
        assert_eq!(memory.vdp().read_vram_u8(0x0003), 0x00);

        memory.step_vdp(2);
        assert_eq!(memory.vdp().read_vram_u8(0x0002), 0xCC);
        assert_eq!(memory.vdp().read_vram_u8(0x0003), 0xDD);
    }

    #[test]
    fn bus_dma_progresses_with_fragmented_single_cycle_steps() {
        let mut rom = vec![0; 0x400];
        rom[0x200] = 0xAA;
        rom[0x201] = 0xBB;
        rom[0x202] = 0xCC;
        rom[0x203] = 0xDD;

        let cart = Cartridge::from_bytes(rom).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        memory.write_u16(0xC00004, 0x8150);
        memory.write_u16(0xC00004, 0x8F02);
        memory.write_u16(0xC00004, 0x9302);
        memory.write_u16(0xC00004, 0x9400);
        memory.write_u16(0xC00004, 0x9500);
        memory.write_u16(0xC00004, 0x9601);
        memory.write_u16(0xC00004, 0x9700);
        memory.write_u16(0xC00004, 0x4000);
        memory.write_u16(0xC00004, 0x0080);

        memory.step_vdp(1);
        assert_eq!(memory.vdp().read_vram_u8(0x0000), 0x00);
        assert_eq!(memory.vdp().read_vram_u8(0x0001), 0x00);

        memory.step_vdp(1);
        assert_eq!(memory.vdp().read_vram_u8(0x0000), 0xAA);
        assert_eq!(memory.vdp().read_vram_u8(0x0001), 0xBB);
        assert_eq!(memory.vdp().read_vram_u8(0x0002), 0x00);
        assert_eq!(memory.vdp().read_vram_u8(0x0003), 0x00);

        memory.step_vdp(1);
        assert_eq!(memory.vdp().read_vram_u8(0x0002), 0x00);
        assert_eq!(memory.vdp().read_vram_u8(0x0003), 0x00);

        memory.step_vdp(1);
        assert_eq!(memory.vdp().read_vram_u8(0x0002), 0xCC);
        assert_eq!(memory.vdp().read_vram_u8(0x0003), 0xDD);
    }

    #[test]
    fn long_bus_dma_does_not_block_vblank_interrupt_timing() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        // reg1: display + DMA enable + V-INT enable
        memory.write_u16(0xC00004, 0x8170);
        memory.write_u16(0xC00004, 0x8F02);
        // DMA length = 0xFFFF words (longer than one NTSC frame worth of DMA slots).
        memory.write_u16(0xC00004, 0x93FF);
        memory.write_u16(0xC00004, 0x94FF);
        memory.write_u16(0xC00004, 0x9500);
        memory.write_u16(0xC00004, 0x9600);
        memory.write_u16(0xC00004, 0x9700);
        memory.write_u16(0xC00004, 0x4000);
        memory.write_u16(0xC00004, 0x0080);

        // Advance roughly one frame in fragmented 1-cycle slices.
        let mut frame_ready = false;
        for _ in 0..130_000 {
            if memory.step_vdp(1) {
                frame_ready = true;
                break;
            }
        }
        assert!(frame_ready, "expected frame boundary during long DMA");
        assert_eq!(memory.vdp().pending_interrupt_level(), Some(6));
        // DMA is still in progress at this point.
        assert_ne!(
            (memory.vdp().register(20), memory.vdp().register(19)),
            (0x00, 0x00)
        );
    }

    #[test]
    fn long_bus_dma_does_not_block_hblank_interrupt_timing() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        // H-INT every line, display+DMA enabled (no V-INT to avoid priority masking).
        memory.write_u16(0xC00004, 0x8010);
        memory.write_u16(0xC00004, 0x8A00);
        memory.write_u16(0xC00004, 0x8150);
        memory.write_u16(0xC00004, 0x8F02);
        // DMA length = 0x0800 words.
        memory.write_u16(0xC00004, 0x9300);
        memory.write_u16(0xC00004, 0x9408);
        memory.write_u16(0xC00004, 0x9500);
        memory.write_u16(0xC00004, 0x9600);
        memory.write_u16(0xC00004, 0x9700);
        memory.write_u16(0xC00004, 0x4000);
        memory.write_u16(0xC00004, 0x0080);

        let mut saw_hint = false;
        for _ in 0..2_000 {
            if memory.vdp().pending_interrupt_level() == Some(4) {
                saw_hint = true;
                break;
            }
            memory.step_vdp(1);
        }
        assert!(saw_hint, "expected H-INT during active DMA transfer");
        // DMA should still be active when the first H-INT appears.
        assert_ne!(
            (memory.vdp().register(20), memory.vdp().register(19)),
            (0x00, 0x00)
        );
    }

    #[test]
    fn bus_dma_updates_source_registers_only_after_transfer_completes() {
        let mut rom = vec![0; 0x400];
        rom[0x200] = 0xAA;
        rom[0x201] = 0xBB;
        rom[0x202] = 0xCC;
        rom[0x203] = 0xDD;

        let cart = Cartridge::from_bytes(rom).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        memory.write_u16(0xC00004, 0x8150);
        memory.write_u16(0xC00004, 0x8F02);
        memory.write_u16(0xC00004, 0x9302);
        memory.write_u16(0xC00004, 0x9400);
        memory.write_u16(0xC00004, 0x9500);
        memory.write_u16(0xC00004, 0x9601);
        memory.write_u16(0xC00004, 0x9700);
        memory.write_u16(0xC00004, 0x4000);
        memory.write_u16(0xC00004, 0x0080);

        // Mid-transfer: source and length registers are not finalized yet.
        memory.step_vdp(2);
        assert_eq!(memory.vdp().register(21), 0x00);
        assert_eq!(memory.vdp().register(22), 0x01);
        assert_eq!(memory.vdp().register(19), 0x02);
        assert_eq!(memory.vdp().register(20), 0x00);

        // Completion finalizes source advance and clears DMA length.
        memory.step_vdp(2);
        assert_eq!(memory.vdp().register(21), 0x02);
        assert_eq!(memory.vdp().register(22), 0x01);
        assert_eq!(memory.vdp().register(19), 0x00);
        assert_eq!(memory.vdp().register(20), 0x00);
    }

    #[test]
    fn bus_dma_advances_source_address_between_transfers() {
        let mut rom = vec![0; 0x600];
        rom[0x200] = 0x11;
        rom[0x201] = 0x22;
        rom[0x202] = 0x33;
        rom[0x203] = 0x44;
        rom[0x204] = 0x55;
        rom[0x205] = 0x66;

        let cart = Cartridge::from_bytes(rom).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        memory.write_u16(0xC00004, 0x8150);
        memory.write_u16(0xC00004, 0x8F02);
        memory.write_u16(0xC00004, 0x9302); // first transfer: 2 words
        memory.write_u16(0xC00004, 0x9400);
        memory.write_u16(0xC00004, 0x9500);
        memory.write_u16(0xC00004, 0x9601);
        memory.write_u16(0xC00004, 0x9700);

        memory.write_u16(0xC00004, 0x4000);
        memory.write_u16(0xC00004, 0x0080);
        let dma_cycles = memory.take_dma_wait_cycles();
        assert_eq!(dma_cycles, 4);
        memory.step_vdp(dma_cycles);
        assert_eq!(memory.take_dma_wait_cycles(), 0);

        // Second transfer: 1 word, source should continue from 0x204.
        memory.write_u16(0xC00004, 0x9301);
        memory.write_u16(0xC00004, 0x9400);
        memory.write_u16(0xC00004, 0x4200); // destination 0x0200
        memory.write_u16(0xC00004, 0x0080);
        let dma_cycles = memory.take_dma_wait_cycles();
        assert_eq!(dma_cycles, 2);
        memory.step_vdp(dma_cycles);
        assert_eq!(memory.take_dma_wait_cycles(), 0);

        assert_eq!(memory.vdp().read_vram_u8(0x0200), 0x55);
        assert_eq!(memory.vdp().read_vram_u8(0x0201), 0x66);
        let trace = memory.dma_trace();
        let second = trace.last().expect("second dma trace");
        let first = trace.iter().rev().nth(1).expect("first dma trace");
        assert_eq!(first.source_addr, 0x00000200);
        assert_eq!(first.dest_addr, 0x0000);
        assert_eq!(first.words, 2);
        assert_eq!(second.source_addr, 0x00000204);
        assert_eq!(second.dest_addr, 0x0200);
        assert_eq!(second.auto_increment, 2);
        assert_eq!(second.words, 1);
        assert_eq!(second.first_word, 0x5566);
        assert_eq!(second.last_word, 0x5566);
    }

    #[test]
    fn runs_vdp_dma_from_work_ram_high_address() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        memory.write_u16(0xFF0000, 0x1234);

        memory.write_u16(0xC00004, 0x8150);
        memory.write_u16(0xC00004, 0x8F02);
        memory.write_u16(0xC00004, 0x9301);
        memory.write_u16(0xC00004, 0x9400);
        // DMA source (word address): 0xFF0000 >> 1 = 0x7F8000.
        memory.write_u16(0xC00004, 0x9500);
        memory.write_u16(0xC00004, 0x9680);
        memory.write_u16(0xC00004, 0x977F);

        memory.write_u16(0xC00004, 0x4000);
        memory.write_u16(0xC00004, 0x0080);
        let dma_cycles = memory.take_dma_wait_cycles();
        assert_eq!(dma_cycles, 2);
        memory.step_vdp(dma_cycles);
        assert_eq!(memory.take_dma_wait_cycles(), 0);

        assert_eq!(memory.vdp().read_vram_u8(0x0000), 0x12);
        assert_eq!(memory.vdp().read_vram_u8(0x0001), 0x34);
    }

    #[test]
    fn runs_vdp_dma_from_68k_bus_into_cram() {
        let mut rom = vec![0; 0x400];
        rom[0x200] = 0x0E;
        rom[0x201] = 0x0E;
        rom[0x202] = 0x02;
        rom[0x203] = 0x22;

        let cart = Cartridge::from_bytes(rom).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        // Enable VDP DMA and configure source length.
        memory.write_u16(0xC00004, 0x8150);
        memory.write_u16(0xC00004, 0x8F02);
        memory.write_u16(0xC00004, 0x9302);
        memory.write_u16(0xC00004, 0x9400);
        memory.write_u16(0xC00004, 0x9500);
        memory.write_u16(0xC00004, 0x9601);
        memory.write_u16(0xC00004, 0x9700); // mode 00: 68k->VDP

        // CRAM write DMA command @ index 0.
        memory.write_u16(0xC00004, 0xC000);
        memory.write_u16(0xC00004, 0x0080);
        let dma_cycles = memory.take_dma_wait_cycles();
        assert_eq!(dma_cycles, 4);
        memory.step_vdp(dma_cycles);
        assert_eq!(memory.take_dma_wait_cycles(), 0);

        assert_eq!(memory.vdp().read_cram_u16(0), 0x0E0E);
        assert_eq!(memory.vdp().read_cram_u16(1), 0x0222);
        let trace = memory.dma_trace();
        let last = trace.last().expect("dma trace entry");
        assert_eq!(last.target, DmaTraceTarget::Cram);
        assert_eq!(last.source_addr, 0x00000200);
        assert_eq!(last.dest_addr, 0x0000);
        assert_eq!(last.auto_increment, 2);
        assert_eq!(last.words, 2);
        assert_eq!(last.first_word, 0x0E0E);
        assert_eq!(last.last_word, 0x0222);
    }

    #[test]
    fn byte_write_to_vdp_control_port_does_not_clear_vblank_status() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid cart");
        let mut memory = MemoryMap::new(cart);

        // reg1: display + v-blank interrupt enable
        memory.write_u16(0xC00004, 0x8160);
        // Advance into vblank interval (before frame wrap).
        let frame_ready = memory.step_vdp(110_000);
        assert!(!frame_ready);

        // First control-port byte write should not perform a hidden status read.
        memory.write_u8(0xC00004, 0x12);
        let status = memory.read_u16(0xC00004);
        assert_ne!(status & 0x0008, 0);
    }
}
