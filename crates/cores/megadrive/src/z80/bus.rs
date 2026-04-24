use super::Z80;
use crate::audio::AudioBus;
use crate::cartridge::Cartridge;
use crate::input::IoBus;
use crate::vdp::Vdp;
use std::sync::OnceLock;

pub(super) fn audio_io_wait_cycles() -> u16 {
    static WAIT_CYCLES: OnceLock<u16> = OnceLock::new();
    *WAIT_CYCLES.get_or_init(|| {
        std::env::var("MEGADRIVE_AUDIO_IO_WAIT_CYCLES")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(2)
            .min(32)
    })
}

const IO_VERSION_ADDR: u32 = 0xA10000;
const IO_PORT1_DATA_ADDR: u32 = 0xA10002;
const IO_PORT2_DATA_ADDR: u32 = 0xA10004;
const IO_PORT1_CTRL_ADDR: u32 = 0xA10008;
const IO_PORT2_CTRL_ADDR: u32 = 0xA1000A;

#[derive(Debug, Clone, Copy, Default, bincode::Encode, bincode::Decode)]
pub(super) struct MdBusState {
    bank_address: u32,
    vdp_data_write_latch: u16,
    vdp_control_write_latch: u16,
}

pub(super) struct Z80Bus<'a> {
    pub(super) audio: &'a mut AudioBus,
    pub(super) cartridge: &'a Cartridge,
    pub(super) work_ram: &'a mut [u8; 0x10000],
    pub(super) vdp: &'a mut Vdp,
    pub(super) io: &'a mut IoBus,
}

impl Z80 {
    pub(super) fn read_byte(&self, addr: u16, bus: &mut Z80Bus<'_>) -> u8 {
        match addr {
            0x0000..=0x3FFF => self.ram[(addr as usize) & 0x1FFF],
            0x4000..=0x5FFF => bus.audio.read_ym2612((addr & 0x03) as u8),
            0x8000..=0xFFFF => self.read_68k_window(addr, bus),
            _ => 0xFF,
        }
    }

    pub(super) fn write_byte(&mut self, addr: u16, value: u8, bus: &mut Z80Bus<'_>) {
        match addr {
            0x0000..=0x3FFF => {
                self.ram[(addr as usize) & 0x1FFF] = value;
            }
            0x4000..=0x5FFF => {
                bus.audio.write_ym2612_from_z80((addr & 0x03) as u8, value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
            }
            0x6000..=0x60FF => self.write_bank_register(value),
            0x7F11 => {
                bus.audio.write_psg_from_z80(value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
            }
            0x8000..=0xFFFF => self.write_68k_window(addr, value, bus),
            _ => {}
        }
    }

    pub(super) fn read_port(&self, port: u16, bus: &mut Z80Bus<'_>) -> u8 {
        match port as u8 {
            // YM2612 status/data ports (low-byte decode).
            0x40..=0x43 => bus.audio.read_ym2612((port as u8) & 0x03),
            // External I/O ports are sparsely used on Mega Drive Z80 side.
            // Return open-bus style value for currently unmodeled inputs.
            _ => 0xFF,
        }
    }

    pub(super) fn write_port(&mut self, port: u16, value: u8, bus: &mut Z80Bus<'_>) {
        match port as u8 {
            // YM2612 address/data ports (low-byte decode).
            0x40..=0x43 => {
                bus.audio.write_ym2612_from_z80((port as u8) & 0x03, value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
            }
            // PSG data port
            0x7F => {
                bus.audio.write_psg_from_z80(value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
            }
            _ => {}
        }
    }

    pub(super) fn write_bank_register(&mut self, value: u8) {
        // Genesis Z80 bank register is a serial latch fed by bit0 writes.
        self.md_bus_state.bank_address =
            (self.md_bus_state.bank_address >> 1) | (((value as u32) & 1) << 23);
        self.md_bus_state.bank_address &= 0x00FF_8000;
    }

    #[cfg(test)]
    pub(super) fn set_bank_address_for_test(&mut self, value: u32) {
        self.md_bus_state.bank_address = value;
    }

    #[cfg(test)]
    pub(super) fn bank_address_for_test(&self) -> u32 {
        self.md_bus_state.bank_address
    }

    fn resolve_68k_window_addr(&self, z80_addr: u16) -> u32 {
        let offset = (z80_addr as u32).wrapping_sub(0x8000) & 0x7FFF;
        (self.md_bus_state.bank_address & 0x00FF_8000) | offset
    }

    fn decode_68k_vdp_local_addr(addr: u32) -> Option<u32> {
        if (0xC00000..=0xDFFFFF).contains(&addr) {
            Some(0xC00000 | (addr & 0x1F))
        } else {
            None
        }
    }

    fn is_68k_psg_addr(addr: u32) -> bool {
        let Some(local) = Self::decode_68k_vdp_local_addr(addr) else {
            return false;
        };
        matches!(local, 0xC00011 | 0xC00013 | 0xC00015 | 0xC00017)
    }

    fn read_68k_window(&self, z80_addr: u16, bus: &mut Z80Bus<'_>) -> u8 {
        let addr = self.resolve_68k_window_addr(z80_addr);
        match addr {
            0x000000..=0x3FFFFF => bus.cartridge.read_u8(addr),
            0xA04000..=0xA04003 => bus.audio.read_ym2612((addr - 0xA04000) as u8),
            0xC00000..=0xDFFFFF => Self::read_vdp_port_byte(addr, bus),
            x if x == IO_VERSION_ADDR || x == IO_VERSION_ADDR + 1 => bus.io.read_version(),
            x if x == IO_PORT1_DATA_ADDR || x == IO_PORT1_DATA_ADDR + 1 => bus.io.read_port1_data(),
            x if x == IO_PORT2_DATA_ADDR || x == IO_PORT2_DATA_ADDR + 1 => bus.io.read_port2_data(),
            x if x == IO_PORT1_CTRL_ADDR || x == IO_PORT1_CTRL_ADDR + 1 => bus.io.read_port1_ctrl(),
            x if x == IO_PORT2_CTRL_ADDR || x == IO_PORT2_CTRL_ADDR + 1 => bus.io.read_port2_ctrl(),
            0xFF0000..=0xFFFFFF => bus.work_ram[(addr - 0xFF0000) as usize],
            _ => 0xFF,
        }
    }

    fn read_vdp_port_byte(addr: u32, bus: &mut Z80Bus<'_>) -> u8 {
        let Some(local) = Self::decode_68k_vdp_local_addr(addr) else {
            return 0xFF;
        };
        let aligned = local & !1;
        let word = match aligned {
            0xC00000 | 0xC00002 => bus.vdp.read_data_port(),
            0xC00004 | 0xC00006 => bus.vdp.read_control_port(),
            0xC00008 | 0xC0000A => bus.vdp.read_hv_counter(),
            _ => return 0xFF,
        };
        if (local & 1) == 0 {
            (word >> 8) as u8
        } else {
            word as u8
        }
    }

    pub(super) fn write_68k_window(&mut self, z80_addr: u16, value: u8, bus: &mut Z80Bus<'_>) {
        let addr = self.resolve_68k_window_addr(z80_addr);
        match addr {
            0xA04000..=0xA04003 => {
                bus.audio
                    .write_ym2612_from_z80((addr - 0xA04000) as u8, value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
            }
            x if x == IO_PORT1_DATA_ADDR || x == IO_PORT1_DATA_ADDR + 1 => {
                bus.io.write_port1_data(value)
            }
            x if x == IO_PORT2_DATA_ADDR || x == IO_PORT2_DATA_ADDR + 1 => {
                bus.io.write_port2_data(value)
            }
            x if x == IO_PORT1_CTRL_ADDR || x == IO_PORT1_CTRL_ADDR + 1 => {
                bus.io.write_port1_ctrl(value)
            }
            x if x == IO_PORT2_CTRL_ADDR || x == IO_PORT2_CTRL_ADDR + 1 => {
                bus.io.write_port2_ctrl(value)
            }
            x if Self::is_68k_psg_addr(x) => {
                bus.audio.write_psg_from_z80(value);
                self.io_wait_cycles = self.io_wait_cycles.saturating_add(audio_io_wait_cycles());
            }
            0xC00000..=0xDFFFFF => self.write_vdp_port_byte(addr, value, bus),
            0xFF0000..=0xFFFFFF => {
                bus.work_ram[(addr - 0xFF0000) as usize] = value;
            }
            _ => {}
        }
    }

    fn write_vdp_port_byte(&mut self, addr: u32, value: u8, bus: &mut Z80Bus<'_>) {
        let Some(local) = Self::decode_68k_vdp_local_addr(addr) else {
            return;
        };
        let aligned = local & !1;
        let immediate_byte_commit = crate::debug_flags::vdp_byte_immediate();
        let low_byte_write = (local & 1) != 0;
        let next = match aligned {
            0xC00000 | 0xC00002 => {
                let current = self.md_bus_state.vdp_data_write_latch;
                let next = if (local & 1) == 0 {
                    ((value as u16) << 8) | (current & 0x00FF)
                } else {
                    (current & 0xFF00) | value as u16
                };
                self.md_bus_state.vdp_data_write_latch = next;
                next
            }
            0xC00004 | 0xC00006 => {
                let current = self.md_bus_state.vdp_control_write_latch;
                let next = if (local & 1) == 0 {
                    ((value as u16) << 8) | (current & 0x00FF)
                } else {
                    (current & 0xFF00) | value as u16
                };
                self.md_bus_state.vdp_control_write_latch = next;
                next
            }
            _ => return,
        };
        match aligned {
            0xC00000 | 0xC00002 => {
                if bus.vdp.trigger_dma_fill_from_data_byte(value) {
                    return;
                }
                if immediate_byte_commit || low_byte_write {
                    bus.vdp.write_data_port(next);
                }
            }
            0xC00004 | 0xC00006 => {
                if immediate_byte_commit || low_byte_write {
                    bus.vdp.write_control_port(next);
                    self.process_pending_vdp_bus_dma(bus);
                }
            }
            _ => {}
        }
    }

    fn process_pending_vdp_bus_dma(&mut self, bus: &mut Z80Bus<'_>) {
        while let Some(request) = bus.vdp.take_bus_dma_request() {
            let mut next_source_addr = request.source_addr & 0x00FF_FFFE;
            for _ in 0..request.words {
                let hi = self.read_dma_source_u8(next_source_addr, bus);
                let lo = self.read_dma_source_u8(next_source_addr.wrapping_add(1), bus);
                bus.vdp.write_data_port(u16::from_be_bytes([hi, lo]));
                next_source_addr = next_source_addr.wrapping_add(2);
            }
            bus.vdp.complete_bus_dma(next_source_addr & 0x00FF_FFFE);

            let dma_wait_cycles = (request.words as u32).saturating_mul(2);
            self.io_wait_cycles = self
                .io_wait_cycles
                .saturating_add(dma_wait_cycles.min(u16::MAX as u32) as u16);
        }
    }

    fn read_dma_source_u8(&self, addr: u32, bus: &mut Z80Bus<'_>) -> u8 {
        let addr = addr & 0x00FF_FFFF;
        match addr {
            0x000000..=0x3FFFFF => bus.cartridge.read_u8(addr),
            0xA00000..=0xA01FFF => self.ram[(addr as usize - 0xA00000) & 0x1FFF],
            0xA04000..=0xA04003 => bus.audio.read_ym2612((addr - 0xA04000) as u8),
            0xC00000..=0xDFFFFF => Self::read_vdp_port_byte(addr, bus),
            x if x == IO_VERSION_ADDR || x == IO_VERSION_ADDR + 1 => bus.io.read_version(),
            x if x == IO_PORT1_DATA_ADDR || x == IO_PORT1_DATA_ADDR + 1 => bus.io.read_port1_data(),
            x if x == IO_PORT2_DATA_ADDR || x == IO_PORT2_DATA_ADDR + 1 => bus.io.read_port2_data(),
            x if x == IO_PORT1_CTRL_ADDR || x == IO_PORT1_CTRL_ADDR + 1 => bus.io.read_port1_ctrl(),
            x if x == IO_PORT2_CTRL_ADDR || x == IO_PORT2_CTRL_ADDR + 1 => bus.io.read_port2_ctrl(),
            0xFF0000..=0xFFFFFF => bus.work_ram[(addr - 0xFF0000) as usize],
            _ => 0xFF,
        }
    }
}
