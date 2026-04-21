use super::Bus;
use crate::cpu::CpuBus;

impl Bus {
    fn read_oam_dma_byte(&mut self, src: u16) -> u8 {
        match src {
            0x0000..=0x1FFF => self.memory.read(src),
            0x6000..=0x7FFF => {
                if let Some(ref cartridge) = self.cartridge {
                    cartridge.read_prg_ram(src)
                } else {
                    0
                }
            }
            0x8000..=0xFFFF => {
                if let Some(ref mut cartridge) = self.cartridge {
                    cartridge.read_prg_cpu(src)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    fn start_oam_dma(&mut self, data: u8) {
        // OAM DMA: Copy 256 bytes from CPU page to PPU OAM
        let base = (data as u16) << 8;
        let oam_addr = self.ppu.get_oam_addr();
        for i in 0u16..256 {
            let byte = self.read_oam_dma_byte(base + i);
            let oam_dst = oam_addr.wrapping_add(i as u8);
            self.ppu.write_oam_data(oam_dst, byte);
        }
        self.dma_in_progress = true;
        self.dma_cycles = 513;
    }
}

impl CpuBus for Bus {
    fn on_reset(&mut self) {
        if let Some(ref mut cartridge) = self.cartridge {
            cartridge.on_reset();
        }
    }

    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.memory.read(addr),
            0x2000..=0x3FFF => {
                let mirrored = 0x2000 + (addr & 0x07);
                // $2002 (PPUSTATUS) is latency-sensitive: the vblank flag
                // is set for only a narrow PPU-cycle window around scanline
                // 241 cycle 1, and on real hardware the actual read happens
                // on the last cycle of its CPU instruction (LDA/BIT abs is
                // 4 cycles, with the read on cycle 4). Our CPU model is
                // atomic per instruction, so pre-advance 3 CPU cycles of
                // PPU/APU timing before returning the status. Any LDA/BIT
                // targeting $2002 is >= 4 cycles, so we never overshoot.
                if mirrored == 0x2002 {
                    self.prepay_cpu_cycle();
                    self.prepay_cpu_cycle();
                    self.prepay_cpu_cycle();
                }
                self.ppu.read_register(mirrored, self.cartridge.as_ref())
            }
            0x4000..=0x4013 | 0x4015 => self.apu.read_register(addr),
            0x4016 => self.read_controller1(),
            0x4017 => self.read_controller2(),
            0x4020..=0x5FFF => {
                if let Some(ref cartridge) = self.cartridge {
                    cartridge.read_prg_low(addr)
                } else {
                    0
                }
            }
            0x6000..=0x7FFF => {
                if let Some(ref cartridge) = self.cartridge {
                    cartridge.read_prg_ram(addr)
                } else {
                    0
                }
            }
            0x8000..=0xFFFF => {
                if let Some(ref mut cartridge) = self.cartridge {
                    cartridge.read_prg_cpu(addr)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1FFF => {
                self.memory.write(addr, data);
            }
            0x2000..=0x3FFF => {
                let mirrored = 0x2000 + (addr & 0x07);
                self.ppu
                    .write_register(mirrored, data, self.cartridge.as_mut());
            }
            0x4000..=0x4013 | 0x4015 | 0x4017 => {
                self.apu.write_register(addr, data);
            }
            0x4014 => {
                self.start_oam_dma(data);
            }
            0x4016 => {
                self.write_controller_strobe(data);
            }
            0x4020..=0xFFFF => {
                if let Some(ref mut cartridge) = self.cartridge {
                    match addr {
                        0x4020..=0x5FFF => {
                            cartridge.write_prg(addr, data);
                        }
                        0x6000..=0x7FFF => {
                            cartridge.write_prg_ram(addr, data);
                        }
                        0x8000..=0xFFFF => {
                            cartridge.write_prg(addr, data);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}
