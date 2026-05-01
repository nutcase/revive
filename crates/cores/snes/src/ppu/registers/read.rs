use crate::ppu::Ppu;

impl Ppu {
    pub(crate) fn read(&mut self, addr: u16) -> u8 {
        if matches!(addr, 0x3C | 0x3D) && self.slhv_latch_pending_dots > 0 {
            // Our scheduler does not advance a dot between back-to-back CPU MMIO reads.
            // Real games often do $2137 immediately followed by $213C/$213D, so realize
            // the pending SLHV latch on the first counter read.
            self.slhv_latch_pending_dots = 0;
            self.latch_hv_counters();
        }
        match addr {
            0x34 => (self.mode7_mul_result & 0xFF) as u8, // product bit7-0
            0x35 => ((self.mode7_mul_result >> 8) & 0xFF) as u8, // product bit15-8
            0x36 => ((self.mode7_mul_result >> 16) & 0xFF) as u8, // product bit23-16
            0x37 => {
                // $2137 (SLHV) - latch H/V counters on read.
                // On read: counter_latch = 1 (always).
                // The returned value is open bus on real hardware.
                // Latch occurs 1 dot after the read (timing-sensitive ROMs rely on this).
                self.slhv_latch_pending_dots = 1;
                0
            }
            0x38 => {
                // OAMDATAREAD ($2138)
                // SNESdev wiki:
                // - $2102/$2103 set OAM *word* address, and internal OAM address becomes (word<<1).
                // - $2138 reads from the internal OAM *byte* address and increments it by 1.
                // - High table (0x200..0x21F) repeats for internal addresses 0x200..0x3FF.
                if !self.can_read_oam_now() {
                    return 0;
                }
                let internal = self.oam_internal_addr & 0x03FF;
                let mapped = if internal < 0x200 {
                    internal
                } else {
                    0x200 | (internal & 0x001F)
                };
                let v = self.oam.get(mapped as usize).copied().unwrap_or(0);
                self.oam_internal_addr = (internal + 1) & 0x03FF;
                self.refresh_oam_eval_base_from_internal_addr();
                v
            }
            0x39 | 0x3A => {
                // VRAM data read ($2139/$213A): one-word latch behavior.
                // - Reading returns the current latch byte.
                // - On the incrementing access (VMAIN bit7 selects low/high), the latch is reloaded
                //   from the current VMADD *before* VMADD is incremented.
                let ret = if addr == 0x39 {
                    self.vram_read_buf_lo
                } else {
                    self.vram_read_buf_hi
                };

                let inc_on_high = (self.vram_mapping & 0x80) != 0;
                let should_inc = (addr == 0x39 && !inc_on_high) || (addr == 0x3A && inc_on_high);
                if should_inc {
                    self.reload_vram_read_latch();
                    self.vram_addr = self.vram_addr.wrapping_add(self.vram_increment);
                }

                ret
            }
            0x3B => {
                // CGRAM Read ($213B): two-step read like write path.
                // Returns low byte first, then high (with bit7 masked), and increments address after high.
                if !self.can_read_cgram_now() {
                    return 0;
                }
                let base = (self.cgram_addr as usize) * 2;
                if !self.cgram_read_second {
                    self.cgram_read_second = true;
                    if base < self.cgram.len() {
                        self.cgram[base]
                    } else {
                        0
                    }
                } else {
                    self.cgram_read_second = false;
                    let hi = if base + 1 < self.cgram.len() {
                        self.cgram[base + 1] & 0x7F
                    } else {
                        0
                    };
                    self.cgram_addr = self.cgram_addr.wrapping_add(1);
                    hi
                }
            }
            0x3C => {
                // OPHCT ($213C) - Latched horizontal counter (2-step read: low then high bit)
                let v = if !self.ophct_second {
                    (self.hv_latched_h & 0x00FF) as u8
                } else {
                    ((self.hv_latched_h >> 8) & 0x01) as u8
                };
                self.ophct_second = !self.ophct_second;
                v
            }
            0x3D => {
                // OPVCT ($213D) - Latched vertical counter (2-step read: low then high bit)
                let v = if !self.opvct_second {
                    (self.hv_latched_v & 0x00FF) as u8
                } else {
                    ((self.hv_latched_v >> 8) & 0x01) as u8
                };
                self.opvct_second = !self.opvct_second;
                v
            }
            0x3E => {
                // STAT77 - PPU Status Flag and Version
                // trm-vvvv
                // t = time over, r = range over, m = master/slave (always 0 here), v = version.
                const STAT77_VER: u8 = 0x01;
                let mut v = 0u8;
                // SNESdev wiki:
                // bit7: Time over flag (sprite tile fetch overflow, >34 tiles on scanline)
                // bit6: Range over flag (sprite overflow, >32 sprites on scanline)
                if self.sprite_overflow_latched {
                    v |= 0x40;
                }
                if self.sprite_time_over_latched {
                    v |= 0x80;
                }
                v | (STAT77_VER & 0x0F)
            }
            0x3F => {
                // STAT78 - PPU Status Flag and Version
                // fl-pvvvv
                // f = interlace field (toggles every VBlank)
                // l = external latch flag (set on HV latch, cleared on read when $4201 bit7=1)
                // p = PAL (0 on NTSC)
                // v = version
                const STAT78_VER: u8 = 0x03;
                let mut v = 0u8;
                if self.interlace_field {
                    v |= 0x80;
                }
                if self.stat78_latch_flag {
                    v |= 0x40;
                }
                // NTSC: bit4 stays 0
                v |= STAT78_VER & 0x0F;

                // Side effect: reset OPHCT/OPVCT high/low selectors.
                self.ophct_second = false;
                self.opvct_second = false;

                // Side effect: counter_latch = 0.
                self.stat78_latch_flag = false;
                v
            }
            _ => 0,
        }
    }
}
