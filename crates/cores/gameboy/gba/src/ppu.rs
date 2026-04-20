use crate::bus::GbaBus;
use crate::state::{StateReader, StateWriter};

const CYCLES_PER_SCANLINE: u32 = 1232;
const HBLANK_START_CYCLE: u32 = 960;
const VDRAW_LINES: u32 = 160;
const TOTAL_LINES: u32 = 228;

pub const GBA_FRAME_CYCLES: u32 = 280_896;

/// Events produced by a single PPU step.
///
/// The caller is responsible for triggering DMA / IRQs based on these flags.
pub struct PpuStepResult {
    pub frame_ready: bool,
    pub vblank_entered: bool,
    pub hblank_entered: bool,
    pub vcounter_match_entered: bool,
    /// Set when the PPU enters the draw period of a new visible scanline
    /// (line 1..159).  Line 0 is handled by the caller before the frame loop.
    pub scanline_entered: Option<u16>,
}

#[derive(Debug, Default)]
pub struct GbaPpu {
    line: u16,
    line_cycle: u16,
    was_vblank: bool,
    was_hblank: bool,
    was_vcounter_match: bool,
    last_vcount: u16,
}

impl GbaPpu {
    pub fn reset(&mut self) {
        self.line = 0;
        self.line_cycle = 0;
        self.was_vblank = false;
        self.was_hblank = false;
        self.was_vcounter_match = false;
        self.last_vcount = 0;
    }

    pub fn serialize_state(&self, w: &mut StateWriter) {
        w.write_u16(self.line);
        w.write_u16(self.line_cycle);
        w.write_bool(self.was_vblank);
        w.write_bool(self.was_hblank);
        w.write_bool(self.was_vcounter_match);
        w.write_u16(self.last_vcount);
    }

    pub fn deserialize_state(&mut self, r: &mut StateReader) -> Result<(), &'static str> {
        self.line = r.read_u16()?;
        self.line_cycle = r.read_u16()?;
        self.was_vblank = r.read_bool()?;
        self.was_hblank = r.read_bool()?;
        self.was_vcounter_match = r.read_bool()?;
        self.last_vcount = r.read_u16()?;
        Ok(())
    }

    pub fn step(&mut self, cycles: u32, bus: &mut GbaBus) -> PpuStepResult {
        let total_line_cycles = u32::from(self.line_cycle) + cycles;
        let line_advance = total_line_cycles / CYCLES_PER_SCANLINE;
        self.line_cycle = (total_line_cycles % CYCLES_PER_SCANLINE) as u16;

        let old_line = u32::from(self.line);
        let new_line = old_line + line_advance;
        let frame_ready = new_line >= TOTAL_LINES;
        self.line = (new_line % TOTAL_LINES) as u16;

        let vcount = self.line;
        let vblank = u32::from(vcount) >= VDRAW_LINES;
        let hblank = u32::from(self.line_cycle) >= HBLANK_START_CYCLE;
        let dispstat = bus.dispstat();
        let lyc = ((dispstat >> 8) & 0x00FF) as u16;
        let vcounter_match = vcount == lyc;

        // Signal the start of a visible scanline. The frame loop decides when
        // to snapshot per-line IO state so both the offline renderer and the
        // per-scanline renderer can latch at the same timing point.
        let scanline_entered = if vcount != self.last_vcount
            && u32::from(vcount) < VDRAW_LINES
            && !(frame_ready && vcount == 0)
        {
            Some(vcount)
        } else {
            None
        };

        if vcount != self.last_vcount
            || vblank != self.was_vblank
            || hblank != self.was_hblank
            || vcounter_match != self.was_vcounter_match
        {
            bus.set_lcd_status(vcount, vblank, hblank, vcounter_match);
            self.last_vcount = vcount;
        }

        let vblank_entered = !self.was_vblank && vblank;
        let hblank_entered = !self.was_hblank && hblank;
        let vcounter_match_entered = !self.was_vcounter_match && vcounter_match;

        self.was_vblank = vblank;
        self.was_hblank = hblank;
        self.was_vcounter_match = vcounter_match;

        PpuStepResult {
            frame_ready,
            vblank_entered,
            hblank_entered,
            vcounter_match_entered,
            scanline_entered,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::{IRQ_HBLANK, IRQ_VBLANK, IRQ_VCOUNT};

    /// Helper: run ppu.step and apply DMA/IRQ just like the real frame loop.
    fn step_and_handle(ppu: &mut GbaPpu, cycles: u32, bus: &mut GbaBus) -> PpuStepResult {
        let result = ppu.step(cycles, bus);
        if result.vblank_entered {
            bus.trigger_vblank_dma();
            let dispstat = bus.dispstat();
            if (dispstat & (1 << 3)) != 0 {
                bus.request_irq(IRQ_VBLANK);
            }
        }
        if result.hblank_entered {
            bus.trigger_hblank_dma();
            let dispstat = bus.dispstat();
            if (dispstat & (1 << 4)) != 0 {
                bus.request_irq(IRQ_HBLANK);
            }
        }
        if result.vcounter_match_entered {
            let dispstat = bus.dispstat();
            if (dispstat & (1 << 5)) != 0 {
                bus.request_irq(IRQ_VCOUNT);
            }
        }
        result
    }

    #[test]
    fn updates_vcount_and_vblank_irq() {
        let mut ppu = GbaPpu::default();
        let mut bus = GbaBus::default();
        bus.reset();
        // Enable VBlank IRQ in DISPSTAT.
        bus.write16(0x0400_0004, 1 << 3);

        // Jump to first VBlank line.
        let result = step_and_handle(&mut ppu, 160 * CYCLES_PER_SCANLINE, &mut bus);
        assert!(!result.frame_ready);
        assert_eq!(bus.read16(0x0400_0006), 160);
        assert_ne!(bus.read16(0x0400_0202) & IRQ_VBLANK, 0);
    }

    #[test]
    fn vcount_match_irq_fires_when_lyc_matches() {
        let mut ppu = GbaPpu::default();
        let mut bus = GbaBus::default();
        bus.reset();
        // Enable VCount IRQ and set LYC=7.
        bus.write16(0x0400_0004, (1 << 5) | (7 << 8));

        step_and_handle(&mut ppu, 7 * CYCLES_PER_SCANLINE, &mut bus);
        assert_ne!(bus.read16(0x0400_0202) & IRQ_VCOUNT, 0);
    }

    #[test]
    fn hblank_edge_triggers_hblank_dma() {
        let mut ppu = GbaPpu::default();
        let mut bus = GbaBus::default();
        bus.reset();

        bus.write32(0x0300_0000, 0xDEAD_BEEF);
        bus.write32(0x0400_00D4, 0x0300_0000); // DMA3SAD
        bus.write32(0x0400_00D8, 0x0200_0000); // DMA3DAD
        bus.write16(0x0400_00DC, 1); // DMA3CNT_L
        bus.write16(0x0400_00DE, 0xA400); // enable + 32-bit + HBlank timing

        assert_eq!(bus.read32(0x0200_0000), 0);
        step_and_handle(&mut ppu, HBLANK_START_CYCLE, &mut bus);
        assert_eq!(bus.read32(0x0200_0000), 0xDEAD_BEEF);
    }
}
