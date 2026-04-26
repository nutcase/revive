use super::*;

impl Bus {
    pub fn clear(&mut self) {
        self.ram.fill(0);
        self.large_hucard_mapper = false;
        self.large_hucard_latch = 0;
        self.large_hucard_bank_mask = 0;
        self.io.fill(0);
        self.io_port.reset();
        self.interrupt_disable = 0;
        self.interrupt_request = 0;
        self.timer.reset();
        self.vdc.reset();
        self.psg.reset();
        self.vce.reset();
        self.audio_phi_accumulator = 0;
        self.audio_psg_accumulator = TransientU64(0);
        self.audio_buffer.clear();
        self.reset_audio_diagnostics();
        self.cpu_vdc_vce_penalty_cycles = TransientU64(0);
        self.cpu_high_speed_hint = TransientBool(false);
        self.vce_palette_flicker.0.clear();
        self.sprite_vram_snapshot.0.clear();
        self.framebuffer.fill(0);
        self.frame_ready = false;
        self.cart_ram.fill(0);
        self.bram_unlocked = TransientBool(false);
        self.video_output_enabled = TransientBool(true);
        self.current_display_width = 256;
        self.current_display_height = DEFAULT_DISPLAY_HEIGHT;
        self.current_display_x_offset = TransientUsize(0);
        self.current_display_y_offset = 0;
        self.bg_opaque.fill(false);
        self.bg_priority.fill(false);
        self.sprite_line_counts.fill(0);
        self.vdc.clear_sprite_overflow();
        #[cfg(debug_assertions)]
        {
            self.debug_force_ds_after = TransientU64(0);
        }
        #[cfg(feature = "trace_hw_writes")]
        {
            self.st0_lock_window = 0;
        }
    }

    pub fn load_rom_image(&mut self, data: Vec<u8>) {
        self.rom = data;
        self.large_hucard_mapper = false;
        self.large_hucard_latch = 0;
        self.large_hucard_bank_mask = 0;
        for idx in 0..NUM_BANKS {
            self.update_mpr(idx);
        }
    }

    pub fn enable_large_hucard_mapper(&mut self) {
        let selectable_windows = self
            .rom_pages()
            .saturating_sub(LARGE_HUCARD_MAPPER_WINDOW_PAGES)
            / LARGE_HUCARD_MAPPER_WINDOW_PAGES;
        if selectable_windows == 0 {
            self.large_hucard_mapper = false;
            self.large_hucard_latch = 0;
            self.large_hucard_bank_mask = 0;
            self.rebuild_mpr_mappings();
            return;
        }

        self.large_hucard_mapper = true;
        self.large_hucard_latch = 0;
        self.large_hucard_bank_mask = selectable_windows.saturating_sub(1).min(0x0F) as u8;
        self.rebuild_mpr_mappings();
    }

    pub fn map_bank_to_ram(&mut self, bank: usize, page: usize) {
        if bank < NUM_BANKS {
            let pages = self.total_ram_pages();
            let page_index = if pages == 0 { 0 } else { page % pages };
            self.mpr[bank] = 0xF8u8.saturating_add(page_index as u8);
            self.update_mpr(bank);
        }
    }

    pub fn map_bank_to_rom(&mut self, bank: usize, rom_bank: usize) {
        if bank < NUM_BANKS {
            let pages = self.rom_pages();
            let page_index = if pages == 0 { 0 } else { rom_bank % pages };
            self.mpr[bank] = page_index as u8;
            self.update_mpr(bank);
        }
    }

    pub fn set_mpr(&mut self, index: usize, value: u8) {
        if index < NUM_BANKS {
            if index == 1 && Self::env_force_mpr1_hardware() {
                #[cfg(feature = "trace_hw_writes")]
                eprintln!(
                    "  MPR1 force-hardware active: ignoring write {:02X}, keeping FF",
                    value
                );
                self.mpr[1] = 0xFF;
                self.update_mpr(1);
                return;
            }
            self.mpr[index] = value;
            self.update_mpr(index);
            #[cfg(feature = "trace_hw_writes")]
            eprintln!("  MPR{index} <= {:02X} -> {:?}", value, self.banks[index]);
        }
    }

    pub fn rebuild_mpr_mappings(&mut self) {
        for idx in 0..NUM_BANKS {
            self.update_mpr(idx);
        }
    }

    pub(crate) fn post_load_fixup(&mut self) {
        if !self.large_hucard_mapper && self.rom_pages() >= LARGE_HUCARD_MAPPER_THRESHOLD_PAGES {
            self.enable_large_hucard_mapper();
        } else if self.large_hucard_mapper {
            self.large_hucard_latch &= 0x0F;
        }
        self.audio_phi_accumulator = 0;
        self.cpu_vdc_vce_penalty_cycles = TransientU64(0);
        self.cpu_high_speed_hint = TransientBool(false);
        self.vce_palette_flicker.0.clear();
        self.sprite_vram_snapshot.0.clear();
        self.audio_psg_accumulator = TransientU64(0);
        self.audio_buffer.clear();
        self.audio_total_phi_cycles = TransientU64(0);
        self.audio_total_generated_samples = TransientU64(0);
        self.audio_total_drained_samples = TransientU64(0);
        self.audio_total_drain_calls = TransientU64(0);
        self.bram_unlocked = TransientBool(false);
        self.video_output_enabled = TransientBool(true);
        self.frame_ready = false;
        self.current_display_x_offset = TransientUsize(0);
        self.bg_opaque.fill(false);
        self.bg_priority.fill(false);
        self.sprite_line_counts.fill(0);
        self.psg.post_load_fixup();
        self.vdc.post_load_fixup();
        self.refresh_vdc_irq();
    }

    pub fn mpr(&self, index: usize) -> u8 {
        self.mpr[index]
    }

    pub fn mpr_array(&self) -> [u8; NUM_BANKS] {
        let mut out = [0u8; NUM_BANKS];
        out.copy_from_slice(&self.mpr);
        out
    }

    pub fn rom_page_count(&self) -> usize {
        self.rom.len() / PAGE_SIZE
    }

    pub fn configure_cart_ram(&mut self, size: usize) {
        if size == 0 {
            self.cart_ram.clear();
        } else if self.cart_ram.len() != size {
            self.cart_ram = vec![0; size];
        } else {
            self.cart_ram.fill(0);
        }
        for idx in 0..NUM_BANKS {
            self.update_mpr(idx);
        }
    }

    pub fn cart_ram_size(&self) -> usize {
        self.cart_ram.len()
    }

    pub fn set_joypad_input(&mut self, state: u8) {
        self.io_port.input = state;
    }

    pub fn cart_ram(&self) -> Option<&[u8]> {
        if self.cart_ram.is_empty() {
            None
        } else {
            Some(&self.cart_ram)
        }
    }

    pub fn cart_ram_mut(&mut self) -> Option<&mut [u8]> {
        if self.cart_ram.is_empty() {
            None
        } else {
            Some(&mut self.cart_ram)
        }
    }

    pub fn bram(&self) -> &[u8] {
        &self.bram
    }

    pub fn bram_mut(&mut self) -> &mut [u8] {
        &mut self.bram
    }

    pub fn bram_unlocked(&self) -> bool {
        *self.bram_unlocked
    }

    pub fn work_ram(&self) -> &[u8] {
        let base = self.mpr1_ram_base();
        &self.ram[base..base + PAGE_SIZE]
    }

    pub fn work_ram_mut(&mut self) -> &mut [u8] {
        let base = self.mpr1_ram_base();
        &mut self.ram[base..base + PAGE_SIZE]
    }

    fn mpr1_ram_base(&self) -> usize {
        let mpr1 = self.mpr[1];
        if (0xF8..=0xFD).contains(&mpr1) {
            let ram_pages = self.total_ram_pages().max(1);
            let logical = (mpr1 - 0xF8) as usize % ram_pages;
            logical * PAGE_SIZE
        } else {
            0
        }
    }

    pub fn load_cart_ram(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if self.cart_ram.is_empty() {
            return Err("cart RAM not present");
        }
        if self.cart_ram.len() != data.len() {
            return Err("cart RAM size mismatch");
        }
        self.cart_ram.copy_from_slice(data);
        Ok(())
    }

    pub fn load_bram(&mut self, data: &[u8]) -> Result<(), &'static str> {
        let normalized = Self::normalize_bram_image(data)?;
        self.bram.copy_from_slice(&normalized);
        Ok(())
    }

    fn normalize_bram_image(data: &[u8]) -> Result<Vec<u8>, &'static str> {
        let mut bram = match data.len() {
            BRAM_SIZE => data.to_vec(),
            BRAM_PAGE_DUMP_SIZE => data[..BRAM_SIZE].to_vec(),
            _ => return Err("BRAM size mismatch"),
        };
        Self::repair_bram_header_if_blank(&mut bram);
        Ok(bram)
    }

    fn repair_bram_header_if_blank(bram: &mut [u8]) {
        if bram.len() < BRAM_FORMAT_HEADER.len() {
            return;
        }
        if bram[..BRAM_FORMAT_HEADER.len()]
            .iter()
            .all(|&byte| byte == 0)
        {
            bram[..BRAM_FORMAT_HEADER.len()].copy_from_slice(&BRAM_FORMAT_HEADER);
        }
    }
}
