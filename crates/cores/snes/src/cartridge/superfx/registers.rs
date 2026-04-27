use super::*;

impl SuperFx {
    pub fn read_register(&mut self, offset: u16, mdr: u8) -> u8 {
        match offset {
            0x3000..=0x301F => {
                let reg_index = ((offset - 0x3000) / 2) as usize;
                let word = self.regs[reg_index];
                if (offset & 1) == 0 {
                    word as u8
                } else {
                    (word >> 8) as u8
                }
            }
            0x3030 => {
                let value = self.sfr as u8;
                if trace_superfx_sfr_enabled() {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static CNT: OnceLock<AtomicU32> = OnceLock::new();
                    let n = CNT
                        .get_or_init(|| AtomicU32::new(0))
                        .fetch_add(1, Ordering::Relaxed);
                    if n < 32 {
                        println!(
                            "[SFX-SFR] raw={:04X} running={} read_low={:02X}",
                            self.sfr, self.running as u8, value
                        );
                    }
                }
                value
            }
            0x3100..=0x32FF => self.cache_ram[(offset - 0x3100) as usize],
            0x3031 => {
                let value = (self.sfr >> 8) as u8;
                self.sfr &= !SFR_IRQ_BIT;
                value
            }
            0x3033 => self.bramr & 0x01,
            0x3034 => self.pbr,
            0x3036 => self.rombr & 0x7F,
            0x3038 => self.scbr,
            0x3039 => self.clsr & 0x01,
            0x303A => self.scmr & 0x3F,
            0x303B => self.vcr,
            0x303C => self.rambr & 0x03,
            0x303E => ((self.cbr & 0xFFF0) as u8) | (mdr & 0x0F),
            0x303F => (self.cbr >> 8) as u8,
            _ => mdr,
        }
    }

    #[inline]
    pub fn observed_sfr_low(&self) -> u8 {
        self.sfr as u8
    }

    pub fn write_register(&mut self, offset: u16, value: u8) {
        self.write_register_with_rom(offset, value, &[]);
    }

    pub fn write_register_with_rom(&mut self, offset: u16, value: u8, rom: &[u8]) {
        match offset {
            0x3000..=0x301F => {
                let reg_index = ((offset - 0x3000) / 2) as usize;
                let mut word = self.regs[reg_index];
                if (offset & 1) == 0 {
                    word = (word & 0xFF00) | value as u16;
                } else {
                    word = (word & 0x00FF) | ((value as u16) << 8);
                }
                self.write_reg(reg_index, word);
                if reg_index == 14 {
                    // bsnes updates the ROM buffer pipeline immediately on any
                    // CPU-side R14 write. Preserve that pending reload across
                    // the later GO/start path instead of clearing it via
                    // prepare_start_execution().
                    self.schedule_rom_buffer_reload();
                    self.r14_modified = false;
                }
                if reg_index == 15 && (offset & 1) != 0 {
                    self.invoke_cpu_start(rom);
                }
            }
            0x3100..=0x32FF => {
                self.cache_write(offset, value);
            }
            0x3030 => {
                self.sfr = (self.sfr & 0xFF00) | value as u16;
                self.sync_condition_flags_from_sfr();
                self.apply_sfr_side_effects(rom);
            }
            0x3031 => {
                self.sfr = (self.sfr & 0x00FF) | ((value as u16) << 8);
                self.sync_condition_flags_from_sfr();
                self.apply_sfr_side_effects(rom);
            }
            0x3033 => self.bramr = value & 0x01,
            0x3034 => {
                self.pbr = value & 0x7F;
                self.cache_valid_mask = 0;
            }
            0x3037 => self.cfgr = value & 0x80,
            0x3038 => self.scbr = value,
            0x3039 => self.clsr = value & 0x01,
            0x303A => self.scmr = value & 0x3F,
            _ => {}
        }
    }
}
