use super::*;

impl Ppu {
    pub fn write_register(
        &mut self,
        addr: u16,
        data: u8,
        cartridge: Option<&mut crate::cartridge::Cartridge>,
    ) {
        match addr {
            0x2000 => {
                let old_nmi_enable = self.control.contains(PpuControl::NMI_ENABLE);
                self.control = PpuControl::from_bits_truncate(data);
                if let Some(cart) = cartridge {
                    cart.notify_ppuctrl_mmc5(data);
                }

                // Update nametable select bits in t register
                self.t = (self.t & 0xF3FF) | ((data as u16 & 0x03) << 10);

                // NMI edge detection: 0->1 while VBlank is set triggers immediate NMI
                let new_nmi_enable = self.control.contains(PpuControl::NMI_ENABLE);
                if !old_nmi_enable && new_nmi_enable && self.status.contains(PpuStatus::VBLANK) {
                    self.pending_nmi = true;
                }
            }
            0x2001 => {
                self.mask = PpuMask::from_bits_truncate(data);
                self.rendering_enabled = self.mask.contains(PpuMask::BG_ENABLE)
                    || self.mask.contains(PpuMask::SPRITE_ENABLE);
                if let Some(cart) = cartridge {
                    cart.notify_ppumask_mmc5(data);
                }
            }
            0x2003 => {
                self.oam_addr = data;
            }
            0x2004 => {
                self.oam[self.oam_addr as usize] = data;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            0x2005 => {
                if !self.w {
                    self.x = data & 0x07;
                    self.t = (self.t & 0xFFE0) | ((data as u16) >> 3);
                    self.w = true;
                } else {
                    self.t = (self.t & 0x8C1F)
                        | (((data as u16) & 0x07) << 12)
                        | (((data as u16) >> 3) << 5);
                    self.w = false;
                }
            }
            0x2006 => {
                if !self.w {
                    self.t = (self.t & 0x00FF) | (((data & 0x3F) as u16) << 8);
                    self.w = true;
                } else {
                    self.t = (self.t & 0xFF00) | data as u16;
                    self.v = self.t;
                    self.w = false;
                }
            }
            0x2007 => {
                let write_v = Self::normalize_register_vram_addr(self.v);
                if write_v >= 0x3F00 {
                    let mirrored_addr = Self::mirrored_palette_addr(write_v);
                    self.palette[mirrored_addr] = data;
                } else if (0x2000..0x3000).contains(&write_v) {
                    // Nametable write
                    let addr = (write_v - 0x2000) as usize;
                    let nt_index = (addr >> 10) & 3;
                    let offset = addr & 0x3FF;

                    if offset < 1024 {
                        let physical_nt = self.resolve_nametable(nt_index, cartridge.as_deref());

                        if let Some(cart) = cartridge {
                            cart.write_nametable_byte(
                                physical_nt,
                                offset,
                                &mut self.nametable,
                                data,
                            );
                        } else {
                            self.nametable[physical_nt & 1][offset] = data;
                        }
                    }
                } else if write_v < 0x2000 {
                    // CHR write (for CHR RAM)
                    if let Some(cart) = cartridge {
                        cart.write_chr(write_v, data);
                    }
                    self.increment_register_vram_addr();
                    return;
                }

                self.increment_register_vram_addr();
            }
            _ => {}
        }
    }
}
