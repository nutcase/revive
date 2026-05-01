use super::Ppu;

impl Ppu {
    pub fn latch_hv_counters(&mut self) {
        // Latch current H/V counters. Writing $2137 always updates the latched values.
        // STAT78 bit6 (latch flag) is set until $213F is read (which clears it).
        // H/V counters are 9-bit values on real hardware.
        self.set_hv_latch(self.scanline, self.cycle);
    }

    pub fn latch_hv_counters_one_dot_later(&mut self) {
        self.latch_hv_counters_after_master_cycles(4);
    }

    pub fn latch_hv_counters_after_master_cycles(&mut self, master_cycles: u64) {
        let mut scanline = self.scanline;
        let mut cycle = self.cycle as u64 + master_cycles / 4;
        loop {
            let dots_this_line = self.dots_this_scanline(scanline) as u64;
            if cycle < dots_this_line {
                break;
            }
            cycle -= dots_this_line;
            scanline = scanline.wrapping_add(1);
            if scanline >= self.scanlines_per_frame() {
                scanline = 0;
            }
        }
        self.set_hv_latch(scanline, cycle as u16);
    }

    fn set_hv_latch(&mut self, scanline: u16, cycle: u16) {
        self.hv_latched_h = cycle & 0x01FF;
        self.hv_latched_v = scanline & 0x01FF;
        // STAT78 latch flag: set when counters are latched.
        self.stat78_latch_flag = true;
        // Reset OPHCT/OPVCT read selectors so the next read returns the low byte.
        self.ophct_second = false;
        self.opvct_second = false;

        if crate::debug_flags::trace_burnin_ext_latch() {
            use std::sync::atomic::{AtomicU32, Ordering};
            static CNT: AtomicU32 = AtomicU32::new(0);
            let n = CNT.fetch_add(1, Ordering::Relaxed);
            if n < 1024 && !crate::debug_flags::quiet() {
                println!(
                    "[BURNIN-EXT][LATCH] sl={} cyc={} -> OPHCT={:03} OPVCT={:03} flag={} wio_en={}",
                    self.scanline,
                    self.cycle,
                    self.hv_latched_h,
                    self.hv_latched_v,
                    self.stat78_latch_flag as u8,
                    self.wio_latch_enable as u8
                );
            }
        }
    }

    pub fn request_wrio_hv_latch(&mut self) {
        // WRIO ($4201) external latch is documented as latching 1 dot later than a $2137 read.
        // We schedule the latch so it fires after the next two dots advance.
        self.wio_latch_pending_dots = 2;
        if crate::debug_flags::trace_burnin_ext_latch() && !crate::debug_flags::quiet() {
            use std::sync::atomic::{AtomicU32, Ordering};
            static CNT: AtomicU32 = AtomicU32::new(0);
            let n = CNT.fetch_add(1, Ordering::Relaxed);
            if n < 256 {
                println!(
                    "[BURNIN-EXT][WRIO-LATCH-REQ] sl={} cyc={} pending_dots={}",
                    self.scanline, self.cycle, self.wio_latch_pending_dots
                );
            }
        }
    }

    pub fn set_wio_latch_enable(&mut self, enabled: bool) {
        self.wio_latch_enable = enabled;
    }
}
