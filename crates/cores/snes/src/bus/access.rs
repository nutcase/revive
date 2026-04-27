use super::Bus;

impl Bus {
    pub fn reset_cpu_profile(&mut self) {
        self.cpu_profile_read_ns = 0;
        self.cpu_profile_write_ns = 0;
        self.cpu_profile_bus_cycle_ns = 0;
        self.cpu_profile_tick_ns = 0;
        self.cpu_profile_read_count = 0;
        self.cpu_profile_write_count = 0;
        self.cpu_profile_bus_cycle_count = 0;
        self.cpu_profile_tick_count = 0;
        self.cpu_profile_read_bank_ns = [0; 256];
        self.cpu_profile_read_bank_count = [0; 256];
    }

    pub fn take_cpu_profile(&mut self) -> (u64, u64, u64, u64, u32, u32, u32, u32) {
        let snapshot = (
            self.cpu_profile_read_ns,
            self.cpu_profile_write_ns,
            self.cpu_profile_bus_cycle_ns,
            self.cpu_profile_tick_ns,
            self.cpu_profile_read_count,
            self.cpu_profile_write_count,
            self.cpu_profile_bus_cycle_count,
            self.cpu_profile_tick_count,
        );
        self.reset_cpu_profile();
        snapshot
    }

    pub fn top_cpu_read_banks(&self, limit: usize) -> Vec<(u8, u64, u32)> {
        let mut entries: Vec<(u8, u64, u32)> = self
            .cpu_profile_read_bank_ns
            .iter()
            .enumerate()
            .filter_map(|(bank, &ns)| {
                let count = self.cpu_profile_read_bank_count[bank];
                (ns != 0 && count != 0).then_some((bank as u8, ns, count))
            })
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.cmp(&a.2)));
        entries.truncate(limit);
        entries
    }

    /// Cold path: consolidated debug trace checks for read_u8.
    #[cold]
    #[inline(never)]
    pub(super) fn read_u8_trace(&mut self, addr: u32, bank: u32, offset: u16) {
        // Trace BRK/IRQ/NMI vector reads
        if bank == 0x00
            && (0xFFE0..=0xFFFF).contains(&offset)
            && crate::debug_flags::trace_vectors()
        {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNT_VEC: AtomicU32 = AtomicU32::new(0);
            let n = COUNT_VEC.fetch_add(1, Ordering::Relaxed);
            if n < 32 {
                let raw = self.read_rom_lohi(bank, offset);
                println!(
                    "[VEC] read {:02X}:{:04X} -> {:02X} mdr={:02X}",
                    bank, offset, raw, self.mdr
                );
            }
        }
        // Trace HVBJOY reads
        if offset == 0x4212 && crate::debug_flags::trace_4212() {
            use std::sync::atomic::{AtomicU32, Ordering};
            static READ_COUNT_4212: AtomicU32 = AtomicU32::new(0);
            let idx = READ_COUNT_4212.fetch_add(1, Ordering::Relaxed);
            if idx < 32 {
                println!(
                    "[TRACE4212] addr={:06X} bank={:02X} offset={:04X} MDR=0x{:02X}",
                    addr, bank, offset, self.mdr
                );
            }
        }
        // Trace SA-1 status reg reads ($2300/$2301)
        if offset == 0x2300 || offset == 0x2301 {
            let trace_sfr = crate::debug_flags::trace_sfr();
            let trace_sfr_values = crate::debug_flags::trace_sfr_values();
            if trace_sfr || trace_sfr_values {
                use std::sync::atomic::{AtomicU32, Ordering};
                static READ_COUNT_SFR: AtomicU32 = AtomicU32::new(0);
                let idx = READ_COUNT_SFR.fetch_add(1, Ordering::Relaxed);
                if idx < 16 {
                    let val = if trace_sfr_values {
                        let reg = offset - 0x2200;
                        Some(self.read_sa1_register_scpu(reg))
                    } else {
                        None
                    };
                    if let Some(v) = val {
                        println!(
                            "[TRACE_SFR] addr={:06X} bank={:02X} offset={:04X} val=0x{:02X}",
                            addr, bank, offset, v
                        );
                    } else {
                        println!(
                            "[TRACE_SFR] addr={:06X} bank={:02X} offset={:04X}",
                            addr, bank, offset
                        );
                    }
                }
            }
        }
    }

    #[inline]
    fn dma_a_bus_is_mmio_blocked(addr: u32) -> bool {
        let bank = ((addr >> 16) & 0xFF) as u8;
        let off = (addr & 0xFFFF) as u16;
        // SNESdev wiki: DMA cannot access A-bus addresses that overlap MMIO registers:
        // $2100-$21FF, $4000-$41FF, $4200-$421F, $4300-$437F (in system banks).
        //
        // These MMIO ranges are only mapped in banks $00-$3F and $80-$BF; in other banks
        // the same low addresses typically map to ROM/RAM and are accessible.
        if !((0x00..=0x3F).contains(&bank) || (0x80..=0xBF).contains(&bank)) {
            return false;
        }
        matches!(
            off,
            0x2100..=0x21FF | 0x4000..=0x41FF | 0x4200..=0x421F | 0x4300..=0x437F
        )
    }

    #[inline]
    pub(super) fn dma_read_a_bus(&mut self, addr: u32) -> u8 {
        if Self::dma_a_bus_is_mmio_blocked(addr) {
            // Open bus (MDR) - do not trigger side-effects.
            self.mdr
        } else {
            self.read_u8(addr)
        }
    }

    #[inline]
    pub(super) fn dma_write_a_bus(&mut self, addr: u32, value: u8) {
        if Self::dma_a_bus_is_mmio_blocked(addr) {
            // Ignore writes to MMIO addresses on the A-bus (hardware blocks DMA access).
            return;
        }
        self.write_u8(addr, value);
    }
}
