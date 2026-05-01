use super::*;

impl SuperFx {
    pub(super) fn read_bus_mapped_byte(&self, rom: &[u8], bank: u8, addr: u16) -> Option<u8> {
        let bank = bank & 0x7F;
        let full_addr = ((bank as usize) << 16) | addr as usize;

        if (full_addr & 0xE0_0000) == 0x60_0000 {
            return self.game_ram.get(full_addr % self.game_ram.len()).copied();
        }
        if rom.is_empty() {
            return None;
        }

        // Match bsnes SuperFX bus mapping:
        // - $00-$3F:0000-FFFF => 32KB mirrored LoROM pages
        // - $40-$5F:0000-FFFF => linear 64KB ROM windows
        let offset = if (full_addr & 0xE0_0000) == 0x40_0000 {
            full_addr
        } else {
            ((full_addr & 0x3F_0000) >> 1) | (full_addr & 0x7FFF)
        };
        rom.get(offset % rom.len()).copied()
    }

    pub(super) fn read_program_source_byte(&self, rom: &[u8], bank: u8, addr: u16) -> Option<u8> {
        self.read_bus_mapped_byte(rom, bank, addr)
    }

    pub(super) fn read_data_source_byte(&self, rom: &[u8], bank: u8, addr: u16) -> Option<u8> {
        self.read_bus_mapped_byte(rom, bank, addr)
    }

    pub(super) fn cache_offset_for_addr(&self, addr: u16) -> Option<usize> {
        let offset = addr.wrapping_sub(self.cbr) as usize;
        (offset < CACHE_RAM_SIZE).then_some(offset)
    }

    pub(super) fn fill_cache_line(&mut self, rom: &[u8], bank: u8, addr: u16) {
        let Some(offset) = self.cache_offset_for_addr(addr) else {
            return;
        };
        let line_start_offset = offset & !0x0F;
        let line_index = line_start_offset >> 4;
        let line_start_addr = self.cbr.wrapping_add(line_start_offset as u16);
        for i in 0..16 {
            let cache_idx = line_start_offset + i;
            self.cache_ram[cache_idx] = self
                .read_program_source_byte(rom, bank, line_start_addr.wrapping_add(i as u16))
                .unwrap_or(0);
        }
        self.cache_valid_mask |= 1u32 << line_index;
    }

    pub(super) fn read_program_rom_byte(&mut self, rom: &[u8], bank: u8, addr: u16) -> Option<u8> {
        if (bank & 0x60) == 0x60 {
            self.sync_ram_buffer();
        }
        if let Some(offset) = self.cache_offset_for_addr(addr) {
            let line_index = offset >> 4;
            if (self.cache_valid_mask & (1u32 << line_index)) == 0 {
                self.fill_cache_line(rom, bank, addr);
            }
            return Some(self.cache_ram[offset]);
        }
        self.read_program_source_byte(rom, bank, addr)
    }

    pub(super) fn read_data_rom_byte(&mut self, rom: &[u8]) -> Option<u8> {
        // GETB/GETC read from the ROM buffer without modifying R14.
        // Match bsnes more closely: R14 writes only mark the buffer dirty
        // during instruction execution, and the buffer is refreshed once the
        // instruction completes or on demand before the next GETB/GETC read.
        self.refresh_rom_buffer_if_needed(rom)?;
        Some(self.rom_buffer)
    }

    pub(super) fn ram_addr_with_bank(&self, bank: u8, addr: u16) -> Option<usize> {
        if self.game_ram.is_empty() {
            None
        } else {
            let bank_base = ((bank & 0x03) as usize) << 16;
            Some((bank_base + addr as usize) % self.game_ram.len())
        }
    }

    pub(super) fn ram_addr(&self, addr: u16) -> Option<usize> {
        self.ram_addr_with_bank(self.rambr, addr)
    }

    pub(super) fn peek_ram_byte(&self, addr: u16) -> u8 {
        self.ram_addr(addr)
            .map(|idx| self.game_ram[idx])
            .unwrap_or(0xFF)
    }

    pub(super) fn sync_ram_buffer(&mut self) {
        if !self.ram_buffer_pending {
            return;
        }
        let bank = self.ram_buffer_pending_bank;
        let addr = self.ram_buffer_pending_addr;
        let data = self.ram_buffer_pending_data;
        self.ram_buffer_pending = false;
        self.write_ram_byte_immediate_with_bank(bank, addr, data);
    }

    pub(super) fn ram_word_after_byte_write(
        &self,
        word_addr: u16,
        touched_addr: u16,
        value: u8,
    ) -> u16 {
        let lo_addr = word_addr;
        let hi_addr = word_addr ^ 1;
        let lo = if touched_addr == lo_addr {
            value
        } else {
            self.peek_ram_byte(lo_addr)
        };
        let hi = if touched_addr == hi_addr {
            value
        } else {
            self.peek_ram_byte(hi_addr)
        };
        u16::from_le_bytes([lo, hi])
    }

    pub(super) fn read_ram_byte_raw(&mut self, addr: u16) -> u8 {
        self.last_ram_addr = addr;
        let value = self.peek_ram_byte(addr);
        if trace_superfx_exec_frame_matches(u64::from(current_trace_superfx_frame()))
            && trace_superfx_ram_addr_matches(addr)
        {
            eprintln!(
                "[SFX-RAM-R] f={} pc={:02X}:{:04X} op={:02X} r15={:04X} rambr={:02X} addr={:04X} -> {:02X} src=r{} dst=r{} r12={:04X} r13={:04X} r14={:04X}",
                current_trace_superfx_frame(),
                self.current_exec_pbr,
                self.current_exec_pc,
                self.current_exec_opcode,
                self.regs[15],
                self.rambr,
                addr,
                value,
                self.src_reg,
                self.dst_reg,
                self.regs[12],
                self.regs[13],
                self.regs[14],
            );
        }
        value
    }

    pub(super) fn read_ram_byte(&mut self, addr: u16) -> u8 {
        self.sync_ram_buffer();
        self.read_ram_byte_raw(addr)
    }

    pub(super) fn read_ram_word(&mut self, addr: u16) -> u16 {
        self.last_ram_addr = addr;
        let lo = self.read_ram_byte(addr);
        let hi = self.read_ram_byte(addr ^ 1);
        self.last_ram_addr = addr;
        u16::from_le_bytes([lo, hi])
    }

    pub(super) fn read_ram_word_short(&mut self, addr: u16) -> u16 {
        self.last_ram_addr = addr;
        let lo = self.read_ram_byte(addr);
        let hi = self.read_ram_byte(addr.wrapping_add(1));
        self.last_ram_addr = addr;
        u16::from_le_bytes([lo, hi])
    }

    pub(super) fn write_ram_byte_immediate_with_bank(&mut self, bank: u8, addr: u16, value: u8) {
        let value = if starfox_ram_write_debug_override_enabled() {
            self.maybe_force_starfox_continuation_ptr_byte(addr, value)
        } else {
            value
        };
        self.last_ram_addr = addr;
        self.trace_screen_byte_write(addr, value);
        let frame_matches =
            trace_superfx_exec_frame_matches(u64::from(current_trace_superfx_frame()));
        if frame_matches && trace_superfx_ram_addr_matches(addr) {
            eprintln!(
                "[SFX-RAM-W-ADDR] f={} pc={:02X}:{:04X} op={:02X} r15={:04X} rambr={:02X} addr={:04X} <- {:02X} src=r{}({:04X}) dst=r{}({:04X}) r12={:04X} r13={:04X} r14={:04X}",
                current_trace_superfx_frame(),
                self.current_exec_pbr,
                self.current_exec_pc,
                self.current_exec_opcode,
                self.regs[15],
                self.rambr,
                addr,
                value,
                self.src_reg,
                self.reg(self.src_reg),
                self.dst_reg,
                self.reg(self.dst_reg),
                self.regs[12],
                self.regs[13],
                self.regs[14],
            );
        }
        let save_word_eq_matches =
            save_state_at_superfx_ram_word_eq()
                .as_ref()
                .is_none_or(|items| {
                    items.iter().any(|item| {
                        let watched_addr = item.addr;
                        let touched = addr == watched_addr || addr == (watched_addr ^ 1);
                        touched
                            && self.ram_word_after_byte_write(watched_addr, addr, value)
                                == item.value
                    })
                });
        if frame_matches
            && save_state_at_superfx_ram_addr_matches(addr)
            && save_state_at_superfx_ram_byte_eq_matches(addr, value)
            && save_word_eq_matches
        {
            self.save_state_ram_addr_hit_count =
                self.save_state_ram_addr_hit_count.saturating_add(1);
            if self.save_state_ram_addr_hit.is_none()
                && self.save_state_ram_addr_hit_count >= save_state_at_superfx_ram_addr_hit_index()
            {
                self.save_state_ram_addr_hit =
                    Some((self.current_exec_pbr, self.current_exec_pc, addr));
            }
        }
        if env_presence_cached("TRACE_SFX_RAM_WRITES") {
            use std::sync::atomic::{AtomicU32, Ordering};
            static TOTAL: AtomicU32 = AtomicU32::new(0);
            static NZ: AtomicU32 = AtomicU32::new(0);
            static DETAIL: AtomicU32 = AtomicU32::new(0);
            let t = TOTAL.fetch_add(1, Ordering::Relaxed);
            if value != 0 {
                let n = NZ.fetch_add(1, Ordering::Relaxed);
                if n < 32 {
                    let d = DETAIL.fetch_add(1, Ordering::Relaxed);
                    if d < 32 {
                        eprintln!(
                            "[SFX-RAM-W] pbr={:02X} r15={:04X} rambr={:02X} addr={:04X} <- {:02X} (nz#{} total={})",
                            self.pbr, self.regs[15], self.rambr, addr, value, n, t
                        );
                    }
                }
            }
            if t > 0 && t.is_multiple_of(1_000_000) {
                let nz_count = NZ.load(Ordering::Relaxed);
                eprintln!(
                    "[SFX-RAM-W-SUMMARY] total_writes={} non_zero_writes={}",
                    t, nz_count
                );
            }
        }
        let old_value = self
            .ram_addr_with_bank(bank, addr)
            .map(|idx| self.game_ram[idx])
            .unwrap_or(0xFF);
        self.record_low_ram_write(addr, old_value, value);
        if let Some(idx) = self.ram_addr_with_bank(bank, addr) {
            self.game_ram[idx] = value;
        }
    }

    pub(super) fn write_ram_byte(&mut self, addr: u16, value: u8) {
        self.write_ram_byte_immediate_with_bank(self.rambr, addr, value);
    }

    pub(super) fn write_ram_buffer_byte(&mut self, addr: u16, value: u8) {
        let value = if starfox_ram_write_debug_override_enabled() {
            self.maybe_force_starfox_continuation_ptr_byte(addr, value)
        } else {
            value
        };
        self.sync_ram_buffer();
        self.last_ram_addr = addr;
        self.ram_buffer_pending = true;
        self.ram_buffer_pending_bank = self.rambr & 0x03;
        self.ram_buffer_pending_addr = addr;
        self.ram_buffer_pending_data = value;
    }

    pub(super) fn write_ram_word(&mut self, addr: u16, value: u16) {
        let value = if starfox_ram_write_debug_override_enabled() {
            let value = self.maybe_force_starfox_parser_key_from_match_word(addr, value);
            let value = self.maybe_keep_starfox_success_cursor_armed(addr, value);
            self.maybe_force_starfox_continuation_cursor_word(addr, value)
        } else {
            value
        };
        self.last_ram_addr = addr;
        self.trace_screen_word_write(addr, value);
        self.write_ram_byte(addr, value as u8);
        self.write_ram_byte(addr ^ 1, (value >> 8) as u8);
        self.last_ram_addr = addr;
    }

    pub(super) fn write_ram_buffer_word(&mut self, addr: u16, value: u16) {
        let value = if starfox_ram_write_debug_override_enabled() {
            let value = self.maybe_force_starfox_parser_key_from_match_word(addr, value);
            let value = self.maybe_keep_starfox_success_cursor_armed(addr, value);
            self.maybe_force_starfox_continuation_cursor_word(addr, value)
        } else {
            value
        };
        self.last_ram_addr = addr;
        self.trace_screen_word_write(addr, value);
        self.write_ram_buffer_byte(addr, value as u8);
        self.write_ram_buffer_byte(addr ^ 1, (value >> 8) as u8);
        self.last_ram_addr = addr;
    }

    #[cfg(test)]
    pub(super) fn write_ram_word_short(&mut self, addr: u16, value: u16) {
        let value = if starfox_ram_write_debug_override_enabled() {
            let value = self.maybe_force_starfox_parser_key_from_match_word(addr, value);
            let value = self.maybe_keep_starfox_success_cursor_armed(addr, value);
            self.maybe_force_starfox_continuation_cursor_word(addr, value)
        } else {
            value
        };
        self.last_ram_addr = addr;
        self.trace_screen_word_write(addr, value);
        self.write_ram_byte(addr, value as u8);
        self.write_ram_byte(addr.wrapping_add(1), (value >> 8) as u8);
        self.last_ram_addr = addr;
    }

    pub(super) fn write_ram_buffer_word_short(&mut self, addr: u16, value: u16) {
        let value = if starfox_ram_write_debug_override_enabled() {
            let value = self.maybe_force_starfox_parser_key_from_match_word(addr, value);
            let value = self.maybe_keep_starfox_success_cursor_armed(addr, value);
            self.maybe_force_starfox_continuation_cursor_word(addr, value)
        } else {
            value
        };
        self.last_ram_addr = addr;
        self.trace_screen_word_write(addr, value);
        self.write_ram_buffer_byte(addr, value as u8);
        self.write_ram_buffer_byte(addr.wrapping_add(1), (value >> 8) as u8);
        self.last_ram_addr = addr;
    }
}
