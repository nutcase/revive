//! SA-1 co-processor implementation.
//!
//! This module mirrors the main S-CPU implementation but wraps it in a
//! lightweight adapter so both processors can share the same 65C816 core. Most
//! behavioural details are still approximations; the priority is to expose the
//! register interface and scheduling hooks needed by the rest of the emulator.

use crate::cpu::bus::CpuBus;
use crate::cpu::{Cpu, StatusFlags};
use serde::{Deserialize, Serialize};

/// SA-1 control/status register file ($2200-$23FF window as seen by the S-CPU).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registers {
    pub control: u8,       // $2200
    pub sie: u8,           // $2201 S-CPU interrupt enable mirror
    pub sic: u8,           // $2202 S-CPU interrupt clear
    pub reset_vector: u16, // $2203/$2204
    pub nmi_vector: u16,   // $2205/$2206
    pub irq_vector: u16,   // $2207/$2208
    pub scnt: u8,          // $2209 S-CPU control
    pub cie: u8,           // $220A SA-1 interrupt enable (to S-CPU)
    pub cic: u8,           // $220B SA-1 interrupt clear (to S-CPU)
    pub snv: u16,          // $220C/$220D S-CPU NMI vector
    pub siv: u16,          // $220E/$220F S-CPU IRQ vector
    pub tmcnt: u8,         // $2210 timer control (aka CFR in some docs)
    pub ctr: u8,           // $2211 timer counter
    pub h_timer: u16,      // $2212/$2213 H-timer compare
    pub v_timer: u16,      // $2214/$2215 V-timer compare

    pub dma_control: u8,   // $2230 DMA control (DCNT)
    pub ccdma_control: u8, // $2231 char-conversion DMA control (CDMA)
    pub dma_source: u32,   // $2232-$2234 source address
    pub dma_dest: u32,     // $2235-$2237 destination address
    pub dma_length: u16,   // $2238/$2239 transfer length (normal DMA)

    pub math_control: u8,    // $2250 arithmetic control (MCNT)
    pub math_a: u16,         // $2251/$2252 multiplicand/dividend (MA)
    pub math_b: u16,         // $2253/$2254 multiplier/divisor (MB)
    pub math_result: u64,    // $2306-$230A arithmetic result (40-bit)
    pub math_overflow: bool, // $230B arithmetic overflow flag

    pub varlen_control: u8,       // $2258 variable-length control (VBD)
    pub varlen_addr: u32,         // $2259-$225B ROM start address (VDA)
    pub varlen_current_bits: u32, // current bit offset from VDA boundary
    pub varlen_latched_word: u16, // latched VDP value for low/high reads
    pub varlen_latched: bool,

    pub interrupt_enable: u8,  // combined CIE|SIE mask
    pub interrupt_pending: u8, // pending bits delivered to S-CPU
    pub timer_pending: u8,     // pending timer IRQ bits

    pub bwram_select_snes: u8,   // $2224 SNES-side BW-RAM mapping
    pub bwram_select_sa1: u8,    // $2225 SA-1 BW-RAM mapping
    pub sbwe: u8,                // $2226 SNES BW-RAM write enable (bit7)
    pub cbwe: u8,                // $2227 SA-1 BW-RAM write enable (bit7)
    pub bwram_protect: u8,       // $2228 BW-RAM write-protected area (low nibble)
    pub bwram_bitmap_format: u8, // $223F BW-RAM bitmap format (SEL42)
    pub iram_wp_snes: u8,        // $2229 SNES I-RAM write protection mask
    pub iram_wp_sa1: u8,         // $222A SA-1 I-RAM write protection mask
    pub mmc_bank_c: u8,          // $2220 (mirrored in bus)
    pub mmc_bank_d: u8,          // $2221
    pub mmc_bank_e: u8,          // $2222
    pub mmc_bank_f: u8,          // $2223

    pub sfr: u8, // $2300 SA-1 status flags
    #[allow(dead_code)]
    pub status: u8, // read-only mirror for $2300 reads

    pub dma_pending: bool,
    pub ccdma_pending: bool,
    pub ccdma_buffer_ready: bool,
    pub handshake_state: u8,

    // Character conversion buffer ($2240-$224F) and bookkeeping for type2
    pub brf: [u8; 16],
    pub brf_pos: usize,       // next write position (0-15)
    pub brf_tile_offset: u32, // offset from dma_dest for successive tiles
}

impl Default for Registers {
    fn default() -> Self {
        Self {
            // snes9x/hardware default: CONTROL = 0x20 (NMI vector select high)
            control: 0x20,
            sie: 0,
            sic: 0,
            reset_vector: 0,
            nmi_vector: 0,
            irq_vector: 0,
            scnt: 0,
            cie: 0,
            cic: 0,
            snv: 0,
            siv: 0,
            tmcnt: 0,
            ctr: 0,
            h_timer: 0,
            v_timer: 0,
            dma_control: 0,
            ccdma_control: 0,
            dma_source: 0,
            dma_dest: 0,
            dma_length: 0,
            math_control: 0,
            math_a: 0,
            math_b: 0,
            math_result: 0,
            math_overflow: false,
            varlen_control: 0,
            varlen_addr: 0,
            varlen_current_bits: 0,
            varlen_latched_word: 0,
            varlen_latched: false,
            interrupt_enable: 0,
            interrupt_pending: 0,
            timer_pending: 0,
            bwram_select_snes: 0,
            bwram_select_sa1: 0,
            sbwe: 0,
            cbwe: 0,
            bwram_protect: 0,
            bwram_bitmap_format: 0,
            iram_wp_snes: 0xFF,
            iram_wp_sa1: 0xFF,
            // SA-1 MMC default mapping: C=0, D=1, E=2, F=3 (1MB chunks)
            mmc_bank_c: 0,
            mmc_bank_d: 1,
            mmc_bank_e: 2,
            mmc_bank_f: 3,
            sfr: 0,
            status: 0,
            dma_pending: false,
            ccdma_pending: false,
            ccdma_buffer_ready: false,
            handshake_state: 0,
            brf: [0u8; 16],
            brf_pos: 0,
            brf_tile_offset: 0,
        }
    }
}

/// Shared bus adapter used to feed the SA-1 core without borrowing the [`Bus`]
/// mutably for the entire duration of the step.
struct Sa1BusAdapter<'a> {
    bus_ptr: *mut crate::bus::Bus,
    _marker: core::marker::PhantomData<&'a ()>,
}

impl<'a> Sa1BusAdapter<'a> {
    fn new(bus: &'a mut crate::bus::Bus) -> Self {
        Self {
            bus_ptr: bus as *mut crate::bus::Bus,
            _marker: core::marker::PhantomData,
        }
    }

    #[inline]
    unsafe fn bus(&mut self) -> &mut crate::bus::Bus {
        &mut *self.bus_ptr
    }
}

impl<'a> CpuBus for Sa1BusAdapter<'a> {
    fn read_u8(&mut self, addr: u32) -> u8 {
        unsafe { self.bus().sa1_read_u8(addr) }
    }

    fn write_u8(&mut self, addr: u32, value: u8) {
        unsafe { self.bus().sa1_write_u8(addr, value) }
    }

    fn poll_irq(&mut self) -> bool {
        unsafe {
            let bus = self.bus();
            let sa1 = bus.sa1();
            // SNES->SA1 IRQ request (CONTROL bit7) masked by CIE bit7
            if (sa1.registers.control & 0x80) != 0 && (sa1.registers.cie & 0x80) != 0 {
                return true;
            }
            // Timer IRQ (CIE bit6)
            if sa1.registers.timer_pending != 0 && (sa1.registers.cie & 0x40) != 0 {
                return true;
            }
            // DMA/CC-DMA completion to SA-1 CPU (CIE bit5)
            if (sa1.registers.interrupt_pending & Sa1::IRQ_DMA_FLAG) != 0
                && (sa1.registers.cie & 0x20) != 0
            {
                return true;
            }
            false
        }
    }

    fn poll_nmi(&mut self) -> bool {
        unsafe {
            let bus = self.bus();
            let sa1 = bus.sa1();
            // SNES->SA1 NMI request (CONTROL bit4) masked by CIE bit4
            (sa1.registers.control & 0x10) != 0 && (sa1.registers.cie & 0x10) != 0
        }
    }

    fn opcode_memory_penalty(&mut self, addr: u32) -> u8 {
        // Important: do not forward to the main Bus::opcode_memory_penalty().
        // That hook is used to trigger S-CPU MDMAEN timing (after opcode fetch),
        // and must not be consumed by SA-1 opcode fetches during scheduling.
        let _ = addr;
        0
    }
}

/// SA-1 co-processor state wrapper.
pub struct Sa1 {
    pub cpu: Cpu,
    pub registers: Registers,
    pub(crate) boot_vector_applied: bool,
    pub(crate) boot_pb: u8,
    pub(crate) pending_reset: bool,
    pub(crate) hold_reset: bool,
    pub(crate) ipl_ran: bool,
    pub(crate) h_timer_accum: u32,
    pub(crate) v_timer_accum: u32,
    pub(crate) math_cycles_left: u8,
    pub(crate) math_pending_result: u64,
    pub(crate) math_pending_overflow: bool,
}

impl Sa1 {
    // IRQ line bit (S-CPU sees via SFR bit7)
    pub(crate) const IRQ_LINE_BIT: u8 = 0x80;
    // DMA/CC-DMA completion flag (SFR/CFR bit5 equivalent)
    pub(crate) const IRQ_DMA_FLAG: u8 = 0x20;
    const SIGNED_40_MASK: u64 = (1u64 << 40) - 1;
    const SIGNED_40_SIGN: i128 = 1i128 << 39;

    pub fn new() -> Self {
        let mut cpu = Cpu::new();
        cpu.set_emulation_mode(false);
        cpu.set_p(StatusFlags::from_bits_truncate(0x34));
        Self {
            cpu,
            registers: Registers::default(),
            boot_vector_applied: false,
            boot_pb: 0x00,
            pending_reset: false,
            hold_reset: false,
            ipl_ran: false,
            h_timer_accum: 0,
            v_timer_accum: 0,
            math_cycles_left: 0,
            math_pending_result: 0,
            math_pending_overflow: false,
        }
    }

    #[allow(dead_code)]
    pub fn reset(&mut self, vector: u16) {
        self.cpu.reset(vector);
        self.cpu.set_emulation_mode(false);
        self.cpu.set_p(StatusFlags::from_bits_truncate(0x34));
        self.registers = Registers::default();
        self.boot_vector_applied = false;
        self.boot_pb = 0;
        self.pending_reset = false;
        self.hold_reset = false;
        self.ipl_ran = false;
        self.h_timer_accum = 0;
        self.v_timer_accum = 0;
        self.math_cycles_left = 0;
        self.math_pending_result = 0;
        self.math_pending_overflow = false;
    }

    pub(crate) fn apply_pending_reset(&mut self) {
        if self.pending_reset {
            // Apply reset vector requested by S-CPU (SCNT bit7)
            let vector = if self.registers.reset_vector != 0 {
                self.registers.reset_vector
            } else {
                0x0000
            };
            self.cpu.reset(vector);
            self.cpu.set_emulation_mode(false);
            self.cpu.set_p(StatusFlags::from_bits_truncate(0x34));
            self.cpu.set_pc(vector);
            self.cpu.set_pb(self.boot_pb);
            self.boot_vector_applied = true;
            self.pending_reset = false;
            self.ipl_ran = true;
        } else if !self.boot_vector_applied {
            // Accept reset vectors at $0000 as valid (HLE IPL stub uses 0x0000).
            self.cpu.set_pc(self.registers.reset_vector);
            // Use the boot program bank detected/overridden by the mapper.
            self.cpu.set_pb(self.boot_pb);
            self.boot_vector_applied = true;
        }
    }

    #[allow(dead_code)]
    pub fn step(&mut self, bus: &mut crate::bus::Bus) -> u8 {
        self.apply_pending_reset();

        let mut adapter = Sa1BusAdapter::new(bus);
        self.cpu.step_with_bus(&mut adapter)
    }

    pub fn step_batch(&mut self, bus: &mut crate::bus::Bus, max_cycles: u16) -> u16 {
        self.apply_pending_reset();

        let mut adapter = Sa1BusAdapter::new(bus);
        self.cpu.step_multiple_with_bus(&mut adapter, max_cycles)
    }

    #[inline]
    fn update_interrupt_mask(&mut self) {
        // S-CPU side IRQ mask (SIE). CIE is for SA-1 CPU.
        self.registers.interrupt_enable = self.registers.sie;
    }

    #[inline]
    fn ccdma_enabled(&self) -> bool {
        // CC mode (M bit). Some titles program CC-DMA without setting C.
        (self.registers.dma_control & 0x20) != 0
    }

    #[inline]
    pub(crate) fn dma_has_priority(&self) -> bool {
        // DCNT bit6: 0 = SA-1 CPU priority, 1 = DMA/CC-DMA priority.
        (self.registers.dma_control & 0x40) != 0
    }

    #[inline]
    pub(crate) fn dma_busy(&self) -> bool {
        self.registers.dma_pending
            || self.registers.ccdma_pending
            || self.registers.ccdma_buffer_ready
    }

    #[inline]
    pub(crate) fn control_wait(&self) -> bool {
        (self.registers.control & 0x40) != 0
    }

    #[inline]
    pub(crate) fn control_reset(&self) -> bool {
        (self.registers.control & 0x20) != 0
    }

    pub(crate) fn has_pending_wakeup(&self) -> bool {
        let irq = ((self.registers.control & 0x80) != 0 && (self.registers.cie & 0x80) != 0)
            || (self.registers.timer_pending != 0 && (self.registers.cie & 0x40) != 0)
            || ((self.registers.interrupt_pending & Self::IRQ_DMA_FLAG) != 0
                && (self.registers.cie & 0x20) != 0);
        let nmi = (self.registers.control & 0x10) != 0 && (self.registers.cie & 0x10) != 0;
        irq || nmi
    }

    #[inline]
    /// CC-DMA type selector.
    /// 0: linear copy (bitmap as-is)
    /// 1: bitmap -> tile conversion (default)
    /// 2: extended conversion (type2 / semi-automatic)
    pub(crate) fn ccdma_type(&self) -> Option<u8> {
        if !self.ccdma_enabled() {
            None
        } else if (self.registers.dma_control & 0x10) != 0 {
            Some(1) // type1
        } else {
            Some(0) // type0 (default when T=0)
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn dma_source_device(&self) -> u8 {
        self.registers.dma_control & 0x03
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn dma_dest_device(&self) -> u8 {
        (self.registers.dma_control >> 2) & 0x01
    }

    #[inline]
    pub(crate) fn ccdma_color_code(&self) -> Option<u8> {
        match self.registers.ccdma_control & 0x03 {
            // CCNT bits0-1: 0=8bpp, 1=4bpp, 2=2bpp, 3=reserved (treat as 4bpp)
            0 => Some(8),
            1 => Some(4),
            2 => Some(2),
            3 => Some(4),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn ccdma_color_depth_bits(&self) -> Option<u8> {
        self.ccdma_color_code()
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn ccdma_dmacb(&self) -> Option<u8> {
        self.ccdma_color_depth_bits().map(|depth| match depth {
            8 => 0,
            4 => 1,
            2 => 2,
            _ => 1,
        })
    }

    #[inline]
    pub(crate) fn ccdma_virtual_width_shift(&self) -> u8 {
        (self.registers.ccdma_control >> 2) & 0x07
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn ccdma_chars_per_line(&self) -> usize {
        1usize << (self.ccdma_virtual_width_shift().min(5) as usize)
    }

    #[inline]
    fn dma_is_normal(&self) -> bool {
        (self.registers.dma_control & 0x20) == 0
    }

    #[inline]
    #[allow(dead_code)]
    fn is_ccdma_terminated(&self) -> bool {
        // Bit 7 of CCDMA control ($2231) is the DMA enable bit, not terminate
        // When bit 7 is set, CC-DMA should be enabled, not terminated
        // The bit is cleared by hardware when CC-DMA completes
        false // Never consider CC-DMA as "terminated" based on bit 7
    }

    #[allow(dead_code)]
    pub(crate) fn reset_ccdma_state(&mut self) {
        self.registers.ccdma_pending = false;
        self.registers.ccdma_buffer_ready = false;
        self.registers.handshake_state = 0;
        self.registers.brf_pos = 0;
        self.registers.brf.fill(0);
        self.registers.brf_tile_offset = 0;
    }

    #[allow(dead_code)]
    fn maybe_queue_ccdma(&mut self, reason: &str) {
        if !self.ccdma_enabled() {
            return;
        }
        if self.is_ccdma_terminated() {
            return;
        }
        if self.registers.ccdma_pending {
            return;
        }
        let typ = self.ccdma_type();
        // Type2 (BRF-driven) doesn't rely on dma_length; allow queuing even when length=0.
        if self.registers.dma_length == 0 && typ != Some(2) && !self.registers.ccdma_buffer_ready {
            return;
        }
        self.registers.ccdma_pending = true;
        self.registers.ccdma_buffer_ready = false;
        self.registers.handshake_state = 1;
        if crate::debug_flags::trace_sa1_ccdma() {
            self.log_ccdma_state(&format!("queue:{}", reason));
        }
    }

    pub fn is_dma_pending(&self) -> bool {
        self.registers.dma_pending
    }

    pub fn is_ccdma_pending(&self) -> bool {
        self.registers.ccdma_pending
    }

    /// Whether SA-1 is currently asserting an IRQ to the S-CPU.
    pub fn scpu_irq_asserted(&self) -> bool {
        let asserted = (self.registers.interrupt_pending & self.registers.interrupt_enable) != 0;
        if crate::debug_flags::trace_sa1_irq() {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNT: AtomicU32 = AtomicU32::new(0);
            let n = COUNT.fetch_add(1, Ordering::Relaxed);
            if n < 16 {
                println!(
                    "[SA1-IRQ] asserted={} pend=0x{:02X} ien=0x{:02X}",
                    asserted, self.registers.interrupt_pending, self.registers.interrupt_enable
                );
            }
        }
        asserted
    }

    /// Expose DMA mode to callers.
    pub fn dma_is_normal_public(&self) -> bool {
        self.dma_is_normal()
    }

    #[allow(dead_code)]
    pub(crate) fn trace_ccdma_transfer(&self, _stage: &str) {
        // Stub trace to keep bus logging happy; real tracing is gated elsewhere.
    }

    pub(crate) fn log_ccdma_state(&self, reason: &str) {
        if !crate::debug_flags::trace_sa1_ccdma() {
            return;
        }
        use std::sync::atomic::{AtomicU32, Ordering};
        static TRACE_IDX: AtomicU32 = AtomicU32::new(0);
        let idx = TRACE_IDX.fetch_add(1, Ordering::Relaxed) + 1;
        println!(
            "TRACE_SA1_CCDMA[{}] {} ctrl=0x{:02X} cctrl=0x{:02X} src=0x{:06X} dest=0x{:06X} len=0x{:04X} buf_ready={} pending={} handshake={}",
            idx,
            reason,
            self.registers.dma_control,
            self.registers.ccdma_control,
            self.registers.dma_source,
            self.registers.dma_dest,
            self.registers.dma_length,
            self.registers.ccdma_buffer_ready as u8,
            self.registers.ccdma_pending as u8,
            self.registers.handshake_state
        );
    }

    #[allow(dead_code)]
    pub fn pending_scpu_irq_mask(&self) -> u8 {
        self.registers.interrupt_pending & self.registers.interrupt_enable
    }

    #[allow(dead_code)]
    pub fn tick_timers(&mut self, sa1_cycles: u32) {
        if sa1_cycles == 0 {
            return;
        }

        if self.math_cycles_left > 0 {
            if sa1_cycles >= self.math_cycles_left as u32 {
                self.registers.math_result = self.math_pending_result;
                self.registers.math_overflow = self.math_pending_overflow;
                self.math_cycles_left = 0;
            } else {
                self.math_cycles_left = self.math_cycles_left.saturating_sub(sa1_cycles as u8);
            }
        }

        let mut h_triggers = 0u32;
        if (self.registers.tmcnt & 0x01) != 0 && self.registers.h_timer != 0 {
            let period = self.registers.h_timer as u32;
            self.h_timer_accum = self.h_timer_accum.saturating_add(sa1_cycles);
            while self.h_timer_accum >= period {
                self.h_timer_accum -= period;
                h_triggers = h_triggers.saturating_add(1);
                self.registers.timer_pending |= 0x01;
                self.registers.interrupt_pending |= 0x01;
                self.registers.ctr = self.registers.ctr.wrapping_add(1);
            }
        }

        if (self.registers.tmcnt & 0x02) != 0 && self.registers.v_timer != 0 && h_triggers > 0 {
            let period = self.registers.v_timer as u32;
            self.v_timer_accum = self.v_timer_accum.saturating_add(h_triggers);
            while self.v_timer_accum >= period {
                self.v_timer_accum -= period;
                self.registers.timer_pending |= 0x02;
                self.registers.interrupt_pending |= 0x02;
            }
        }
    }

    pub fn complete_dma(&mut self) -> bool {
        self.registers.dma_pending = false;
        self.registers.dma_control &= !0x80;
        // Flag for status (SFR/CFR bit5)
        self.registers.interrupt_pending |= Self::IRQ_DMA_FLAG;
        // Raise IRQ line for S-CPU
        let irq_enabled = (self.registers.interrupt_enable & Self::IRQ_LINE_BIT) != 0;
        if irq_enabled {
            self.registers.interrupt_pending |= Self::IRQ_LINE_BIT;
        }
        if crate::debug_flags::trace_sa1_dma() {
            println!(
                "TRACE_SA1_DMA: complete irq_enabled={} ctrl=0x{:02X} enable=0x{:02X} pending=0x{:02X}",
                irq_enabled,
                self.registers.dma_control,
                self.registers.interrupt_enable,
                self.registers.interrupt_pending
            );
        }
        irq_enabled
    }

    pub fn complete_ccdma(&mut self) -> bool {
        self.registers.ccdma_pending = false;
        self.registers.sfr |= 0x20;
        self.registers.ccdma_control &= !0x80;
        // Flag DMA done
        self.registers.interrupt_pending |= Self::IRQ_DMA_FLAG;
        let irq_enabled = (self.registers.interrupt_enable & Self::IRQ_LINE_BIT) != 0;
        if irq_enabled {
            self.registers.interrupt_pending |= Self::IRQ_LINE_BIT;
        }
        if crate::debug_flags::trace_sa1_dma() {
            println!(
                "TRACE_SA1_DMA: CC-DMA complete irq_enabled={} cctrl=0x{:02X} enable=0x{:02X}",
                irq_enabled, self.registers.ccdma_control, self.registers.interrupt_enable
            );
        }
        irq_enabled
    }

    fn clear_scpu_irq_pending(&mut self, mask: u8) {
        let before = self.registers.interrupt_pending;
        self.registers.interrupt_pending &= !mask;
        if before != self.registers.interrupt_pending && mask & Self::IRQ_DMA_FLAG != 0 {
            self.registers.handshake_state = 0;
        }
    }

    fn write_sie(&mut self, value: u8) {
        self.registers.sie = value;
        self.update_interrupt_mask();
    }

    fn write_cie(&mut self, value: u8) {
        self.registers.cie = value;
        self.update_interrupt_mask();
    }

    fn write_cfr(&mut self, value: u8) {
        self.registers.tmcnt = value;
        if (value & 0x80) != 0 {
            self.h_timer_accum = 0;
            self.v_timer_accum = 0;
            self.registers.timer_pending = 0;
        }
    }

    fn read_sfr_scpu(&self) -> u8 {
        // SFR ($2300) as seen by S-CPU. Layout: I V D N mmmm
        // mmmm : message nibble written by SA-1 CPU via SCNT ($2209)
        let mut sfr = self.registers.scnt & 0x0F;
        // Bit7: SA-1 -> S-CPU IRQ line asserted
        if self.scpu_irq_asserted() {
            sfr |= 0x80;
        }
        // Bit6: S-CPU IRQ vector select (1 = use SIV register)
        if (self.registers.scnt & 0x40) != 0 {
            sfr |= 0x40;
        }
        // Bit5: DMA/CC-DMA completion flag
        if (self.registers.interrupt_pending & Self::IRQ_DMA_FLAG) != 0 {
            sfr |= 0x20;
        }
        // Bit4: S-CPU NMI vector select (1 = use SNV register)
        if (self.registers.scnt & 0x20) != 0 {
            sfr |= 0x10;
        }
        sfr
    }

    #[inline]
    pub(crate) fn decode_varlen_bits(control: u8) -> u32 {
        let bits = (control & 0x0F) as u32;
        if bits == 0 {
            16
        } else {
            bits
        }
    }

    #[inline]
    fn math_result_byte(&self, index: u8) -> u8 {
        ((self.registers.math_result >> (index as u64 * 8)) & 0xFF) as u8
    }

    #[inline]
    fn effective_math_result_signed40(&self) -> i128 {
        let value = if self.math_cycles_left > 0 {
            self.math_pending_result
        } else {
            self.registers.math_result
        };
        if (value & (1u64 << 39)) != 0 {
            (value as i128) - (1i128 << 40)
        } else {
            value as i128
        }
    }

    #[inline]
    fn arithmetic_delay_cycles(mode: u8) -> u8 {
        if mode == 0x01 {
            6
        } else {
            5
        }
    }

    fn execute_arithmetic(&mut self) {
        let mode = self.registers.math_control & 0x03;
        let a = self.registers.math_a as i16 as i64;
        let mut result = self.registers.math_result;
        let mut overflow = false;

        match mode {
            0x00 => {
                let product = a.saturating_mul(self.registers.math_b as i16 as i64);
                result = (product as i128 as u128 as u64) & Self::SIGNED_40_MASK;
            }
            0x01 => {
                let divisor = self.registers.math_b;
                let (quotient, remainder) = if divisor == 0 {
                    (
                        if self.registers.math_a as i16 >= 0 {
                            0xFFFF
                        } else {
                            0x0001
                        },
                        (self.registers.math_a as i16).unsigned_abs(),
                    )
                } else {
                    let q = a / divisor as i64;
                    let r = a % divisor as i64;
                    ((q as i16) as u16, (r & 0xFFFF) as u16)
                };
                result = quotient as u64 | ((remainder as u64) << 16);
            }
            0x02 | 0x03 => {
                let product = a.saturating_mul(self.registers.math_b as i16 as i64) as i128;
                let current = self.effective_math_result_signed40();
                let next = current + product;
                if !(-Self::SIGNED_40_SIGN..Self::SIGNED_40_SIGN).contains(&next) {
                    overflow = true;
                }
                result = (next as u128 as u64) & Self::SIGNED_40_MASK;
            }
            _ => {}
        }

        self.math_pending_result = result;
        self.math_pending_overflow = overflow;
        self.math_cycles_left = Self::arithmetic_delay_cycles(mode);
    }

    pub fn read_register(&mut self, offset: u16) -> u8 {
        match offset {
            0x00 => self.registers.control,
            0x01 => self.registers.sie,
            0x02 => self.registers.sic,
            0x03 => (self.registers.reset_vector & 0xFF) as u8,
            0x04 => (self.registers.reset_vector >> 8) as u8,
            0x05 => (self.registers.nmi_vector & 0xFF) as u8,
            0x06 => (self.registers.nmi_vector >> 8) as u8,
            0x07 => (self.registers.irq_vector & 0xFF) as u8,
            0x08 => (self.registers.irq_vector >> 8) as u8,
            0x09 => self.registers.scnt,
            0x0A => self.registers.cie,
            0x0B => self.registers.cic,
            0x0C => (self.registers.snv & 0xFF) as u8,
            0x0D => (self.registers.snv >> 8) as u8,
            0x0E => (self.registers.siv & 0xFF) as u8,
            0x0F => (self.registers.siv >> 8) as u8,
            0x10 => self.registers.tmcnt,
            0x11 => self.registers.ctr,
            0x12 => (self.registers.h_timer & 0xFF) as u8,
            0x13 => (self.registers.h_timer >> 8) as u8,
            0x14 => (self.registers.v_timer & 0xFF) as u8,
            0x15 => (self.registers.v_timer >> 8) as u8,
            0x20 => self.registers.mmc_bank_c,
            0x21 => self.registers.mmc_bank_d,
            0x22 => self.registers.mmc_bank_e,
            0x23 => self.registers.mmc_bank_f,
            0x24 => self.registers.bwram_select_snes,
            0x25 => self.registers.bwram_select_sa1,
            0x26 => self.registers.sbwe,
            0x27 => self.registers.cbwe,
            0x28 => self.registers.bwram_protect,
            0x3F => self.registers.bwram_bitmap_format,
            0x29 => self.registers.iram_wp_snes,
            0x2A => self.registers.iram_wp_sa1,
            0x30 => self.registers.dma_control,
            0x31 => self.registers.ccdma_control,
            0x32 => (self.registers.dma_source & 0xFF) as u8,
            0x33 => ((self.registers.dma_source >> 8) & 0xFF) as u8,
            0x34 => ((self.registers.dma_source >> 16) & 0xFF) as u8,
            0x35 => (self.registers.dma_dest & 0xFF) as u8,
            0x36 => ((self.registers.dma_dest >> 8) & 0xFF) as u8,
            0x37 => ((self.registers.dma_dest >> 16) & 0xFF) as u8,
            0x38 => (self.registers.dma_length & 0xFF) as u8,
            0x39 => (self.registers.dma_length >> 8) as u8,
            0x40..=0x4F => self.registers.brf[(offset - 0x40) as usize],
            0x50 => self.registers.math_control,
            0x51 => (self.registers.math_a & 0xFF) as u8,
            0x52 => (self.registers.math_a >> 8) as u8,
            0x53 => (self.registers.math_b & 0xFF) as u8,
            0x54 => (self.registers.math_b >> 8) as u8,
            0x58 => self.registers.varlen_control,
            0x59 => (self.registers.varlen_addr & 0xFF) as u8,
            0x5A => ((self.registers.varlen_addr >> 8) & 0xFF) as u8,
            0x5B => ((self.registers.varlen_addr >> 16) & 0xFF) as u8,
            0x100 => {
                // Mirror of SFR for SA-1 side reads (rare). Keep same layout.
                let mut sfr = self.registers.control & 0x0F;
                if self.scpu_irq_asserted() {
                    sfr |= 0x80;
                }
                if (self.registers.scnt & 0x02) != 0 {
                    sfr |= 0x40;
                }
                if (self.registers.interrupt_pending & Self::IRQ_DMA_FLAG) != 0 {
                    sfr |= 0x20;
                }
                if (self.registers.scnt & 0x10) != 0 {
                    sfr |= 0x10;
                }
                sfr
            }
            0x101 => {
                // CFR: SA-1 CPU status (ITDNmmmm) as seen by SA-1
                // Lower nibble: messages from S-CPU (CCNT low nibble)
                let mut cfr = self.registers.control & 0x0F;
                // Bit7: IRQ request from S-CPU (CONTROL bit7)
                if (self.registers.control & 0x80) != 0 {
                    cfr |= 0x80;
                }
                // Bit6: Timer IRQ pending (H/V timers)
                if self.registers.timer_pending != 0 {
                    cfr |= 0x40;
                }
                // Bit5: DMA/CC-DMA completion flag
                if (self.registers.interrupt_pending & Self::IRQ_DMA_FLAG) != 0 {
                    cfr |= 0x20;
                }
                // Bit4: NMI vector select (CONTROL bit4)
                if (self.registers.control & 0x10) != 0 {
                    cfr |= 0x10;
                }
                cfr
            }
            0x106 => self.math_result_byte(0),
            0x107 => self.math_result_byte(1),
            0x108 => self.math_result_byte(2),
            0x109 => self.math_result_byte(3),
            0x10A => self.math_result_byte(4),
            0x10B => {
                if self.registers.math_overflow {
                    0x80
                } else {
                    0x00
                }
            }
            0x10E => self.registers.timer_pending,
            _ => 0,
        }
    }

    fn read_cfr_scpu(&self) -> u8 {
        let mut cfr = self.registers.control & 0x0F;
        if (self.registers.control & 0x80) != 0 {
            cfr |= 0x80;
        }
        if self.registers.timer_pending != 0 {
            cfr |= 0x40;
        }
        if (self.registers.interrupt_pending & Self::IRQ_DMA_FLAG) != 0 {
            cfr |= 0x20;
        }
        if (self.registers.control & 0x10) != 0 {
            cfr |= 0x10;
        }
        cfr
    }

    pub fn read_register_scpu(&mut self, offset: u16, mdr: u8) -> u8 {
        match offset {
            // Writable S-CPU-side control/DMA registers read back their latched values.
            0x00..=0x4F => self.read_register(offset),
            0x100 => self.read_sfr_scpu(),
            0x101 => self.read_cfr_scpu(),
            _ => mdr,
        }
    }

    pub fn write_register_scpu(&mut self, offset: u16, value: u8) {
        if (0x09..=0x0F).contains(&offset) {
            // SA-1-only control region ($2209-$220F)
            return;
        }
        self.write_register(offset, value);
    }

    pub fn write_register_sa1(&mut self, offset: u16, value: u8) {
        if (0x00..=0x08).contains(&offset) {
            // S-CPU-only control region ($2200-$2208)
            return;
        }
        self.write_register(offset, value);
    }

    pub fn write_register(&mut self, offset: u16, value: u8) {
        match offset {
            0x00 => {
                let prev = self.registers.control;
                self.registers.control = value;
                let now_reset = (value & 0x20) != 0;
                let prev_reset = (prev & 0x20) != 0;
                self.hold_reset = now_reset;
                if now_reset && !prev_reset {
                    self.pending_reset = true;
                }
                if std::env::var_os("TRACE_SA1_BOOT").is_some()
                    || std::env::var_os("DEBUG_SA1_SCHEDULER").is_some()
                {
                    println!(
                        "SA-1 $2200 write: control=0x{:02X} (SA1_EN={} IRQ_EN={})",
                        value,
                        value & 0x80,
                        value & 0x01
                    );
                }
            }
            0x01 => self.write_sie(value),
            0x02 => {
                self.registers.sic = value;
                self.clear_scpu_irq_pending(value);
            }
            0x03 => {
                self.registers.reset_vector =
                    (self.registers.reset_vector & 0xFF00) | (value as u16);
                self.boot_vector_applied = false;
                if std::env::var_os("TRACE_SA1_BOOT").is_some()
                    || std::env::var_os("DEBUG_SA1_SCHEDULER").is_some()
                {
                    println!(
                        "SA-1 $2203 write: reset_vector low=0x{:02X}, full=0x{:04X}",
                        value, self.registers.reset_vector
                    );
                }
            }
            0x04 => {
                self.registers.reset_vector =
                    (self.registers.reset_vector & 0x00FF) | ((value as u16) << 8);
                self.boot_vector_applied = false;
                if std::env::var_os("TRACE_SA1_BOOT").is_some()
                    || std::env::var_os("DEBUG_SA1_SCHEDULER").is_some()
                {
                    println!(
                        "SA-1 $2204 write: reset_vector high=0x{:02X}, full=0x{:04X}",
                        value, self.registers.reset_vector
                    );
                }
            }
            0x05 => {
                self.registers.nmi_vector = (self.registers.nmi_vector & 0xFF00) | (value as u16);
            }
            0x06 => {
                self.registers.nmi_vector =
                    (self.registers.nmi_vector & 0x00FF) | ((value as u16) << 8);
            }
            0x07 => {
                self.registers.irq_vector = (self.registers.irq_vector & 0xFF00) | (value as u16);
            }
            0x08 => {
                self.registers.irq_vector =
                    (self.registers.irq_vector & 0x00FF) | ((value as u16) << 8);
            }
            0x09 => {
                // SCNT: S-CPU control register (written by SA-1 CPU)
                // Layout (IS-Nmmmm):
                //  bit7 I : request IRQ to S-CPU
                //  bit6 S : use SIV (IRQ vector select) when set
                //  bit5 N : use SNV (NMI vector select) when set
                //  bit4 - : reserved
                //  bits3-0 : message nibble readable via SFR ($2300)
                if std::env::var_os("TRACE_SA1_BOOT").is_some()
                    || std::env::var_os("DEBUG_SA1_SCHEDULER").is_some()
                {
                    println!(
                        "SA-1 $2209 write: scnt=0x{:02X} (SCPU_IRQ={} IRQ_VEC_SEL={} NMI_VEC_SEL={} msg=0x{:X})",
                        value,
                        (value & 0x80) != 0,
                        (value & 0x40) != 0,
                        (value & 0x20) != 0,
                        value & 0x0F
                    );
                }
                self.registers.scnt = value;

                // Bit7: assert SA-1 -> S-CPU IRQ line (subject to SIE mask)
                if (value & 0x80) != 0 {
                    self.registers.interrupt_pending |= Self::IRQ_LINE_BIT;
                }
                // Bit5/6 merely influence SFR bits; they are reflected when read.
            }
            0x0A => self.write_cie(value),
            0x0B => {
                self.registers.cic = value;
                // Clear pending flags per bits
                if (value & 0x80) != 0 {
                    self.registers.control &= !0x80;
                    self.registers.interrupt_pending &= !Self::IRQ_LINE_BIT;
                }
                if (value & 0x40) != 0 {
                    self.registers.timer_pending = 0;
                }
                if (value & 0x20) != 0 {
                    self.registers.interrupt_pending &= !Self::IRQ_DMA_FLAG;
                }
                self.clear_scpu_irq_pending(value);
            }
            0x0C => self.registers.snv = (self.registers.snv & 0xFF00) | (value as u16),
            0x0D => self.registers.snv = (self.registers.snv & 0x00FF) | ((value as u16) << 8),
            0x0E => self.registers.siv = (self.registers.siv & 0xFF00) | (value as u16),
            0x0F => self.registers.siv = (self.registers.siv & 0x00FF) | ((value as u16) << 8),
            0x10 => self.write_cfr(value),
            0x11 => self.registers.ctr = value,
            0x12 => self.registers.h_timer = (self.registers.h_timer & 0xFF00) | (value as u16),
            0x13 => {
                self.registers.h_timer = (self.registers.h_timer & 0x00FF) | ((value as u16) << 8)
            }
            0x14 => self.registers.v_timer = (self.registers.v_timer & 0xFF00) | (value as u16),
            0x15 => {
                self.registers.v_timer = (self.registers.v_timer & 0x00FF) | ((value as u16) << 8)
            }
            0x20 => self.registers.mmc_bank_c = value & 0x07,
            0x21 => self.registers.mmc_bank_d = value & 0x07,
            0x22 => self.registers.mmc_bank_e = value & 0x07,
            0x23 => self.registers.mmc_bank_f = value & 0x07,
            0x24 => self.registers.bwram_select_snes = value & 0x1F,
            0x25 => {
                let masked = if (value & 0x80) != 0 {
                    // Bit 7 selects the virtual bitmap window; keep bits 0-6.
                    0x80 | (value & 0x7F)
                } else {
                    value & 0x1F
                };
                self.registers.bwram_select_sa1 = masked;
                if crate::debug_flags::trace_sa1_bwram_guard() {
                    println!(
                        "📝 SA-1 $2225 write: value=0x{:02X} (masked=0x{:02X})",
                        value, masked
                    );
                }
            }
            // $2240-$224F: Bitmap Register File (CC-DMA type2 source)
            0x40..=0x4F => {
                let idx = (offset - 0x40) as usize;
                self.registers.brf[idx] = value;
                self.registers.brf_pos = idx + 1;
                // When 16 bytes are written in type2 mode, mark buffer ready
                if self.ccdma_enabled() && self.registers.brf_pos == 16 {
                    self.registers.ccdma_pending = true;
                    self.registers.ccdma_buffer_ready = true;
                    self.registers.handshake_state = 1;
                }
                if crate::debug_flags::trace_sa1_reg() {
                    println!("SA1 BRF W ${:04X} = {:02X}", 0x2240 + idx as u16, value);
                }
            }
            0x26 => {
                self.registers.sbwe = value & 0x80;
                if crate::debug_flags::trace_sa1_bwram_guard() {
                    println!("📝 SA-1 $2226 write: SBWE=0x{:02X}", self.registers.sbwe);
                }
            }
            0x27 => {
                self.registers.cbwe = value & 0x80;
                if crate::debug_flags::trace_sa1_bwram_guard() {
                    println!("📝 SA-1 $2227 write: CBWE=0x{:02X}", self.registers.cbwe);
                }
            }
            0x28 => {
                self.registers.bwram_protect = value & 0x0F;
                if crate::debug_flags::trace_sa1_bwram_guard() {
                    println!(
                        "📝 SA-1 $2228 write: BWPA=0x{:02X}",
                        self.registers.bwram_protect
                    );
                }
            }
            0x3F => {
                // BW-RAM bitmap format: SEL42 (bit7) selects 4bpp/2bpp for bitmap view.
                self.registers.bwram_bitmap_format = value & 0x80;
            }
            0x29 => {
                self.registers.iram_wp_snes = value;
                if crate::debug_flags::trace_sa1_iram_guard() {
                    println!("📝 SA-1 $2229 write: SIWP=0x{:02X}", value);
                }
            }
            0x2A => {
                self.registers.iram_wp_sa1 = value;
                if crate::debug_flags::trace_sa1_iram_guard() {
                    println!("📝 SA-1 $222A write: CIWP=0x{:02X}", value);
                }
            }
            0x30 => {
                let previous = self.registers.dma_control;
                self.registers.dma_control = value;
                if crate::debug_flags::trace_sa1_dma() || crate::debug_flags::trace_sa1_ccdma() {
                    println!(
                        "SA1_DMA: $2230 write value=0x{:02X} prev=0x{:02X} len=0x{:04X} src=0x{:06X} dest=0x{:06X} pending={} ccdma_en={} type={:?}",
                        value,
                        previous,
                        self.registers.dma_length,
                        self.registers.dma_source,
                        self.registers.dma_dest,
                        self.registers.dma_pending as u8,
                        (value & 0x20) != 0,
                        self.ccdma_type()
                    );
                }
                let cc_mode = (value & 0x20) != 0;
                // この時点では開始しない。宛先レジスタ書き込み（$2236/$2237）で開始する。
                self.registers.dma_pending = false;
                self.registers.ccdma_pending = false;
                self.registers.ccdma_buffer_ready = false;
                if cc_mode {
                    self.registers.handshake_state = 0;
                }
                if crate::debug_flags::trace_sa1_dma() {
                    println!(
                        "TRACE_SA1_DMA: $2230 write ctrl=0x{:02X}→0x{:02X} pending={} type={} src=0x{:06X} dest=0x{:06X} len=0x{:04X}",
                        previous,
                        self.registers.dma_control,
                        self.registers.dma_pending,
                        if self.dma_is_normal() { "normal" } else { "cc" },
                        self.registers.dma_source,
                        self.registers.dma_dest,
                        self.registers.dma_length
                    );
                }
            }
            0x31 => {
                let previous = self.registers.ccdma_control;
                self.registers.ccdma_control = value;
                let end_flag = (value & 0x80) != 0;
                if crate::debug_flags::trace_sa1_ccdma() {
                    println!(
                        "SA1_CCDMA: $2231 write value=0x{:02X} prev=0x{:02X} end={} buf_ready={} len=0x{:04X} type={:?} handshake={}",
                        value,
                        previous,
                        end_flag,
                        self.registers.ccdma_buffer_ready as u8,
                        self.registers.dma_length,
                        self.ccdma_type(),
                        self.registers.handshake_state
                    );
                }
                if self.ccdma_enabled() {
                    match self.ccdma_type() {
                        Some(1) => {
                            // Type1 finishes the pixel conversion first, then waits for the
                            // S-CPU to acknowledge the completed DMA by setting bit7.
                            if end_flag && self.registers.handshake_state == 2 {
                                self.complete_ccdma();
                            }
                        }
                        Some(2) => {
                            // Type2: treat bit7 as a start trigger for safety
                            if end_flag {
                                self.registers.ccdma_pending = true;
                                self.registers.ccdma_buffer_ready = true;
                                self.registers.handshake_state = 1;
                            }
                        }
                        _ => {}
                    }
                }
                if crate::debug_flags::trace_sa1_dma() {
                    println!(
                        "TRACE_SA1_DMA: $2231 write ctrl=0x{:02X}→0x{:02X} pending={} start={} buf_ready={} len=0x{:04X}",
                        previous,
                        self.registers.ccdma_control,
                        self.registers.ccdma_pending,
                        end_flag,
                        self.registers.ccdma_buffer_ready,
                        self.registers.dma_length
                    );
                }
            }
            0x32 => {
                self.registers.dma_source = (self.registers.dma_source & 0xFFFF00) | (value as u32);
            }
            0x33 => {
                self.registers.dma_source =
                    (self.registers.dma_source & 0xFF00FF) | ((value as u32) << 8);
            }
            0x34 => {
                self.registers.dma_source =
                    (self.registers.dma_source & 0x00FFFF) | ((value as u32) << 16);
            }
            0x35 => {
                self.registers.dma_dest = (self.registers.dma_dest & 0xFFFF00) | (value as u32);
            }
            0x36 => {
                self.registers.dma_dest =
                    (self.registers.dma_dest & 0xFF00FF) | ((value as u32) << 8);
                // D=0 (IRAM宛て) の場合、ここでDMA/CC-DMAを起動
                let dest_is_bwram = ((self.registers.dma_control >> 2) & 0x01) != 0;
                if !dest_is_bwram && (self.registers.dma_control & 0x80) != 0 {
                    if self.ccdma_enabled() {
                        if !self.registers.ccdma_pending {
                            self.registers.ccdma_pending = true;
                            self.registers.handshake_state = 1;
                            if crate::debug_flags::trace_sa1_dma() {
                                println!(
                                    "SA1_DMA: start CC-DMA (dest IRAM) dcnt=0x{:02X} src=0x{:06X} dest=0x{:06X} len=0x{:04X}",
                                    self.registers.dma_control,
                                    self.registers.dma_source,
                                    self.registers.dma_dest,
                                    self.registers.dma_length
                                );
                            }
                        }
                    } else {
                        self.registers.dma_pending = true;
                        if crate::debug_flags::trace_sa1_dma() {
                            println!(
                                "SA1_DMA: start normal DMA (dest IRAM) dcnt=0x{:02X} src=0x{:06X} dest=0x{:06X} len=0x{:04X}",
                                self.registers.dma_control,
                                self.registers.dma_source,
                                self.registers.dma_dest,
                                self.registers.dma_length
                            );
                        }
                    }
                }
                // CC-DMA can be triggered even if C bit is not set.
                if !dest_is_bwram
                    && self.ccdma_enabled()
                    && !self.registers.ccdma_pending
                    && (self.registers.dma_control & 0x80) == 0
                {
                    self.registers.ccdma_pending = true;
                    self.registers.handshake_state = 1;
                    if crate::debug_flags::trace_sa1_dma() {
                        println!(
                            "SA1_DMA: start CC-DMA (dest IRAM, C=0) dcnt=0x{:02X} src=0x{:06X} dest=0x{:06X} len=0x{:04X}",
                            self.registers.dma_control,
                            self.registers.dma_source,
                            self.registers.dma_dest,
                            self.registers.dma_length
                        );
                    }
                }
            }
            0x37 => {
                self.registers.dma_dest =
                    (self.registers.dma_dest & 0x00FFFF) | ((value as u32) << 16);
                // D=1 (BW-RAM宛て) の場合、ここでDMA/CC-DMAを起動
                let dest_is_bwram = ((self.registers.dma_control >> 2) & 0x01) != 0;
                if dest_is_bwram && (self.registers.dma_control & 0x80) != 0 {
                    if self.ccdma_enabled() {
                        if !self.registers.ccdma_pending {
                            self.registers.ccdma_pending = true;
                            self.registers.handshake_state = 1;
                            if crate::debug_flags::trace_sa1_dma() {
                                println!(
                                    "SA1_DMA: start CC-DMA (dest BWRAM) dcnt=0x{:02X} src=0x{:06X} dest=0x{:06X} len=0x{:04X}",
                                    self.registers.dma_control,
                                    self.registers.dma_source,
                                    self.registers.dma_dest,
                                    self.registers.dma_length
                                );
                            }
                        }
                    } else {
                        self.registers.dma_pending = true;
                        if crate::debug_flags::trace_sa1_dma() {
                            println!(
                                "SA1_DMA: start normal DMA (dest BWRAM) dcnt=0x{:02X} src=0x{:06X} dest=0x{:06X} len=0x{:04X}",
                                self.registers.dma_control,
                                self.registers.dma_source,
                                self.registers.dma_dest,
                                self.registers.dma_length
                            );
                        }
                    }
                }
                // CC-DMA can be triggered even if C bit is not set.
                if dest_is_bwram
                    && self.ccdma_enabled()
                    && !self.registers.ccdma_pending
                    && (self.registers.dma_control & 0x80) == 0
                {
                    self.registers.ccdma_pending = true;
                    self.registers.handshake_state = 1;
                    if crate::debug_flags::trace_sa1_dma() {
                        println!(
                            "SA1_DMA: start CC-DMA (dest BWRAM, C=0) dcnt=0x{:02X} src=0x{:06X} dest=0x{:06X} len=0x{:04X}",
                            self.registers.dma_control,
                            self.registers.dma_source,
                            self.registers.dma_dest,
                            self.registers.dma_length
                        );
                    }
                }
            }
            0x38 => {
                self.registers.dma_length = (self.registers.dma_length & 0xFF00) | (value as u16);
            }
            0x39 => {
                self.registers.dma_length =
                    (self.registers.dma_length & 0x00FF) | ((value as u16) << 8);
            }
            0x50 => {
                self.registers.math_control = value & 0x03;
                if (self.registers.math_control & 0x02) != 0 {
                    self.registers.math_result = 0;
                    self.registers.math_overflow = false;
                }
            }
            0x51 => {
                self.registers.math_a = (self.registers.math_a & 0xFF00) | value as u16;
            }
            0x52 => {
                self.registers.math_a = (self.registers.math_a & 0x00FF) | ((value as u16) << 8);
            }
            0x53 => {
                self.registers.math_b = (self.registers.math_b & 0xFF00) | value as u16;
            }
            0x54 => {
                self.registers.math_b = (self.registers.math_b & 0x00FF) | ((value as u16) << 8);
                self.execute_arithmetic();
            }
            0x58 => {
                self.registers.varlen_control = value & 0x8F;
                if (value & 0x80) == 0 {
                    self.registers.varlen_current_bits = Self::decode_varlen_bits(value);
                }
                self.registers.varlen_latched = false;
            }
            0x59 => {
                self.registers.varlen_addr = (self.registers.varlen_addr & 0xFFFF00) | value as u32;
                self.registers.varlen_control = 0;
                self.registers.varlen_current_bits = 0;
                self.registers.varlen_latched = false;
            }
            0x5A => {
                self.registers.varlen_addr =
                    (self.registers.varlen_addr & 0xFF00FF) | ((value as u32) << 8);
                self.registers.varlen_control = 0;
                self.registers.varlen_current_bits = 0;
                self.registers.varlen_latched = false;
            }
            0x5B => {
                self.registers.varlen_addr =
                    (self.registers.varlen_addr & 0x00FFFF) | ((value as u32) << 16);
                self.registers.varlen_control = 0;
                self.registers.varlen_current_bits = 0;
                self.registers.varlen_latched = false;
            }
            _ => {}
        }
    }
}
