use super::*;

impl Ppu {
    pub fn read_register(
        &mut self,
        addr: u16,
        cartridge: Option<&crate::cartridge::Cartridge>,
    ) -> u8 {
        match addr {
            0x2002 => {
                let status = self.status.bits();

                // Clear VBlank flag after read
                self.status.remove(PpuStatus::VBLANK);

                // Reset write toggle
                self.w = false;

                // NMI suppression: reading $2002 on the exact cycle VBlank is set
                if self.scanline == 241 && self.cycle == 1 {
                    self.nmi_suppressed = true;
                }

                status
            }
            0x2004 => self.oam[self.oam_addr as usize],
            0x2007 => {
                // Super Mario Bros title screen fix: Proper $2007 read implementation
                let data = if self.v >= 0x3F00 {
                    // Palette RAM: Immediate read (no buffering)
                    let mirrored_addr = Self::mirrored_palette_addr(self.v);
                    // Also fill read_buffer with nametable data "underneath" the palette
                    let nt_addr = (self.v & 0x2FFF) as usize;
                    if nt_addr >= 0x2000 {
                        let offset_in_nt = nt_addr - 0x2000;
                        let logical_nt = (offset_in_nt >> 10) & 3;
                        let table = self.resolve_nametable(logical_nt, cartridge);
                        let offset = offset_in_nt & 0x3FF;
                        self.read_buffer = self.read_nametable_byte(table, offset, cartridge);
                    }
                    self.palette[mirrored_addr]
                } else {
                    // All other memory: Buffered read (crucial for SMB)
                    let old_buffer = self.read_buffer;

                    // Update buffer with new data
                    let effective_v = Self::normalize_register_vram_addr(self.v);
                    if (0x2000..0x3000).contains(&effective_v) {
                        // Nametable read with proper mirroring
                        let addr = (effective_v - 0x2000) as usize;
                        let logical_nt = (addr >> 10) & 3;
                        let table = self.resolve_nametable(logical_nt, cartridge);
                        let offset = addr & 0x3FF;
                        self.read_buffer = self.read_nametable_byte(table, offset, cartridge);
                    } else if effective_v < 0x2000 {
                        // CHR-ROM/CHR-RAM read
                        if let Some(cart) = cartridge {
                            self.read_buffer = cart.read_chr(effective_v);
                        } else {
                            self.read_buffer = 0;
                        }
                    } else {
                        self.read_buffer = 0;
                    }

                    old_buffer
                };

                // CRITICAL: Increment VRAM address AFTER read
                self.increment_register_vram_addr();

                data
            }
            _ => 0,
        }
    }
}
