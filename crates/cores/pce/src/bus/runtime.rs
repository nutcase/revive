use super::font::FONT;
use super::*;

impl Bus {
    pub fn write_st_port(&mut self, port: usize, value: u8) {
        self.note_cpu_vdc_vce_penalty();
        self.write_st_port_internal(port, value);
    }

    pub(super) fn write_st_port_internal(&mut self, port: usize, value: u8) {
        let slot_index = port.min(self.st_ports.len().saturating_sub(1));
        if let Some(slot) = self.st_ports.get_mut(slot_index) {
            *slot = value;
        }
        #[cfg(feature = "trace_hw_writes")]
        if Self::env_trace_mpr() {
            use std::fmt::Write as _;
            let mut m = String::new();
            for (i, val) in self.mpr.iter().enumerate() {
                let _ = write!(m, "{}:{:02X} ", i, val);
            }
            eprintln!(
                "  TRACE MPR pc={:04X} st{}={:02X} mpr={}",
                self.last_pc_for_trace.unwrap_or(0),
                port,
                value,
                m.trim_end()
            );
        }
        #[cfg(feature = "trace_hw_writes")]
        eprintln!(
            "  ST{port} <= {:02X} (addr={:04X})",
            value, self.vdc.last_io_addr
        );
        match port {
            0 => {
                #[cfg(feature = "trace_hw_writes")]
                if !Self::st0_hold_enabled() {
                    self.vdc.st0_hold_counter = 0;
                }
                #[cfg(feature = "trace_hw_writes")]
                if self.vdc.st0_hold_counter > 0 {
                    if value == self.vdc.selected_register() {
                        self.vdc.st0_hold_counter = self.vdc.st0_hold_counter.saturating_sub(1);
                        let idx = (self.vdc.last_io_addr as usize) & 0xFF;
                        if let Some(slot) = self.vdc.st0_hold_addr_hist.get_mut(idx) {
                            *slot = slot.saturating_add(1);
                        }
                        eprintln!(
                            "  ST0 ignored (hold) pending={:?} phase={:?} value={:02X}",
                            self.vdc.pending_write_register, self.vdc.write_phase, value
                        );
                        return;
                    }
                    self.vdc.st0_hold_counter = 0;
                }
                self.vdc.write_port(0, value)
            }
            1 => {
                #[cfg(feature = "trace_hw_writes")]
                {
                    if Self::st0_hold_enabled() {
                        const HOLD_SPAN: u8 = 8;
                        self.vdc.st0_hold_counter = HOLD_SPAN;
                    } else {
                        self.vdc.st0_hold_counter = 0;
                    }
                }
                self.vdc.write_port(1, value)
            }
            2 => {
                #[cfg(feature = "trace_hw_writes")]
                {
                    if Self::st0_hold_enabled() {
                        const HOLD_SPAN: u8 = 8;
                        self.vdc.st0_hold_counter = HOLD_SPAN;
                    } else {
                        self.vdc.st0_hold_counter = 0;
                    }
                }
                self.vdc.write_port(2, value)
            }
            _ => {}
        }
        #[cfg(feature = "trace_hw_writes")]
        if port == 0 && value == 0x05 {
            self.vdc.pending_traced_register = Some(0x05);
            #[cfg(feature = "trace_hw_writes")]
            eprintln!("  TRACE select R05");
        }
        #[cfg(feature = "trace_hw_writes")]
        if matches!(port, 1 | 2) {
            if let Some(sel) = self.vdc.pending_traced_register.take() {
                #[cfg(feature = "trace_hw_writes")]
                {
                    use std::fmt::Write as _;
                    let mut mpr_buf = String::new();
                    for (i, m) in self.mpr.iter().enumerate() {
                        if i > 0 {
                            mpr_buf.push(' ');
                        }
                        let _ = write!(mpr_buf, "{:02X}", m);
                    }
                    eprintln!(
                        "  TRACE R{:02X} data via ST{} = {:02X} (selected={:02X} pc={:04X} mpr={})",
                        sel,
                        port,
                        value,
                        self.vdc.selected_register(),
                        self.last_pc_for_trace.unwrap_or(0),
                        mpr_buf
                    );
                }
            }
        }
        if self.vdc.take_vram_dma_request() {
            self.perform_vram_dma();
        }
        self.refresh_vdc_irq();
    }

    pub fn read_st_port(&mut self, port: usize) -> u8 {
        self.note_cpu_vdc_vce_penalty();
        self.read_st_port_internal(port)
    }

    fn read_st_port_internal(&mut self, port: usize) -> u8 {
        let value = match port {
            0 => self.vdc.selected_register(),
            1 => self.vdc.read_port(1),
            2 => self.vdc.read_port(2),
            _ => 0,
        };
        let slot_index = port.min(self.st_ports.len().saturating_sub(1));
        if let Some(slot) = self.st_ports.get_mut(slot_index) {
            *slot = value;
        }
        self.refresh_vdc_irq();
        value
    }

    pub fn st_port(&self, port: usize) -> u8 {
        self.st_ports.get(port).copied().unwrap_or(0)
    }

    pub fn vdc_register(&self, index: usize) -> Option<u16> {
        self.vdc.register(index)
    }

    pub fn vdc_status_bits(&self) -> u8 {
        self.vdc.status_bits()
    }

    pub fn vdc_current_scanline(&self) -> u16 {
        self.vdc.scanline
    }

    pub fn vdc_in_vblank(&self) -> bool {
        self.vdc.in_vblank
    }

    pub fn vdc_busy_cycles(&self) -> u32 {
        self.vdc.busy_cycles
    }

    pub fn vdc_map_dimensions(&self) -> (usize, usize) {
        self.vdc.map_dimensions()
    }

    pub fn vdc_vram_word(&self, addr: u16) -> u16 {
        let idx = (addr as usize) & 0x7FFF;
        *self.vdc.vram.get(idx).unwrap_or(&0)
    }

    pub fn vdc_write_vram_direct(&mut self, addr: u16, value: u16) {
        let idx = (addr as usize) & 0x7FFF;
        if let Some(slot) = self.vdc.vram.get_mut(idx) {
            *slot = value;
        }
    }

    #[cfg(test)]
    pub fn sprite_line_counts_for_test(&self) -> &[u8] {
        &self.sprite_line_counts
    }

    pub fn vce_palette_word(&self, index: usize) -> u16 {
        self.vce.palette_word(index)
    }

    pub fn vce_palette_rgb(&self, index: usize) -> u32 {
        self.vce.palette_rgb(index)
    }

    #[cfg(test)]
    pub fn vdc_set_status_for_test(&mut self, mask: u8) {
        self.vdc.raise_status(mask);
        self.refresh_vdc_irq();
    }

    pub fn read_io(&mut self, offset: usize) -> u8 {
        let value = self.read_io_internal(offset);
        self.refresh_vdc_irq();
        value
    }

    pub fn set_video_output_enabled(&mut self, enabled: bool) {
        self.video_output_enabled = TransientBool(enabled);
        if !enabled {
            self.frame_ready = false;
            self.vdc.clear_frame_trigger();
        }
    }

    pub(super) fn capture_sprite_vram_snapshot(&mut self) {
        self.sprite_vram_snapshot.0.clear();
        self.sprite_vram_snapshot
            .0
            .extend_from_slice(&self.vdc.vram);
    }

    pub fn write_io(&mut self, offset: usize, value: u8) {
        self.write_io_internal(offset, value);
        self.refresh_vdc_irq();
    }

    pub fn tick(&mut self, cycles: u32, high_speed: bool) -> bool {
        let phi_cycles = if high_speed {
            cycles
        } else {
            cycles.saturating_mul(4)
        };

        if Self::env_force_timer() {
            self.timer.counter = 0;
            self.interrupt_request |= IRQ_REQUEST_TIMER;
        }

        let previous_in_active_display = self.vdc.in_active_display_period();
        if self.vdc.tick(phi_cycles) {
            self.refresh_vdc_irq();
        }
        if !previous_in_active_display && self.vdc.in_active_display_period() {
            self.capture_sprite_vram_snapshot();
        }

        if self.vdc.in_vblank && self.vdc.cram_pending {
            self.perform_cram_dma();
            self.refresh_vdc_irq();
        }

        if self.vdc.frame_ready() {
            if !*self.video_output_enabled {
                self.vdc.clear_frame_trigger();
                self.frame_ready = false;
            } else {
                self.render_frame_from_vram();
            }
        }

        if self.timer.tick(cycles, high_speed) {
            self.interrupt_request |= IRQ_REQUEST_TIMER;
        }

        if self.psg.tick(cycles) {
            self.raise_irq(IRQ_REQUEST_IRQ2);
        }

        self.enqueue_audio_samples(phi_cycles);
        self.refresh_vdc_irq();
        self.irq_pending()
    }

    #[cfg(feature = "trace_hw_writes")]
    pub fn set_last_pc_for_trace(&mut self, pc: u16) {
        self.last_pc_for_trace = Some(pc);
    }

    pub fn psg_sample(&mut self) -> i16 {
        let psg_cycles = self.psg_cycles_for_host_sample();
        self.psg.render_host_sample(psg_cycles)
    }

    pub fn psg_channel_info(&self, ch: usize) -> (u16, u8, u8, u8) {
        if ch < 6 {
            let c = &self.psg.channels[ch];
            (c.frequency, c.control, c.balance, c.noise_control)
        } else {
            (0, 0, 0, 0)
        }
    }

    pub fn psg_main_balance(&self) -> u8 {
        self.psg.main_balance
    }

    pub fn psg_waveform(&self, ch: usize) -> [u8; 32] {
        let mut out = [0u8; 32];
        if ch < 6 {
            let base = ch * 32;
            out.copy_from_slice(&self.psg.waveform_ram[base..base + 32]);
        }
        out
    }

    pub fn psg_channel_detail(&self, ch: usize) -> (u8, u8, u32, u32, u8) {
        if ch < 6 {
            let c = &self.psg.channels[ch];
            (
                c.wave_pos,
                c.wave_write_pos,
                c.phase,
                c.phase_step,
                c.dda_sample,
            )
        } else {
            (0, 0, 0, 0, 0)
        }
    }

    pub fn timer_info(&self) -> (u8, u8, bool, u32) {
        (
            self.timer.reload,
            self.timer.counter,
            self.timer.enabled,
            self.timer.prescaler,
        )
    }

    pub fn irq_state(&self) -> (u8, u8) {
        (self.interrupt_disable, self.interrupt_request)
    }

    pub fn audio_diagnostics(&self) -> AudioDiagnostics {
        AudioDiagnostics {
            master_clock_hz: MASTER_CLOCK_HZ,
            sample_rate_hz: AUDIO_SAMPLE_RATE,
            total_phi_cycles: *self.audio_total_phi_cycles,
            generated_samples: *self.audio_total_generated_samples,
            drained_samples: *self.audio_total_drained_samples,
            drain_calls: *self.audio_total_drain_calls,
            pending_bus_samples: self.audio_buffer.len(),
            phi_remainder: self.audio_phi_accumulator,
        }
    }

    pub fn reset_audio_diagnostics(&mut self) {
        self.audio_total_phi_cycles = TransientU64(0);
        self.audio_total_generated_samples = TransientU64(0);
        self.audio_total_drained_samples = TransientU64(0);
        self.audio_total_drain_calls = TransientU64(0);
    }

    #[inline]
    pub(super) fn note_cpu_vdc_vce_penalty(&mut self) {
        self.cpu_vdc_vce_penalty_cycles.0 = self.cpu_vdc_vce_penalty_cycles.0.saturating_add(1);
    }

    pub(crate) fn take_cpu_vdc_vce_penalty(&mut self) -> u8 {
        let penalty = self.cpu_vdc_vce_penalty_cycles.0.min(u8::MAX as u64) as u8;
        self.cpu_vdc_vce_penalty_cycles = TransientU64(0);
        penalty
    }

    pub(crate) fn set_cpu_high_speed_hint(&mut self, high_speed: bool) {
        self.cpu_high_speed_hint = TransientBool(high_speed);
    }

    pub(super) fn note_vce_palette_access_flicker(&mut self) {
        let Some(row) = self.vdc.active_row_for_scanline(self.vdc.scanline as usize) else {
            return;
        };
        let line_idx = self.vdc.scanline as usize;
        let line_start = self.vdc.display_start_for_line(line_idx);
        let display_width = self
            .vdc
            .display_width_for_line(line_idx)
            .max(1)
            .min(FRAME_WIDTH);
        let x = line_start
            + ((self.vdc.phi_scaled as usize).saturating_mul(display_width)
                / (VDC_VBLANK_INTERVAL as usize))
                .min(display_width.saturating_sub(1));
        let len = self
            .vce
            .palette_access_stall_pixels(*self.cpu_high_speed_hint)
            .max(1);
        self.vce_palette_flicker
            .0
            .push(PaletteFlickerEvent { row, x, len });
    }

    pub fn take_audio_samples(&mut self) -> Vec<i16> {
        let mut out = Vec::with_capacity(self.audio_buffer.len());
        self.drain_audio_samples_into(&mut out);
        out
    }

    pub fn drain_audio_samples_into(&mut self, out: &mut Vec<i16>) {
        let drained = self.audio_buffer.len() as u64;
        if drained != 0 {
            self.audio_total_drained_samples.0 =
                self.audio_total_drained_samples.0.saturating_add(drained);
            self.audio_total_drain_calls.0 = self.audio_total_drain_calls.0.saturating_add(1);
        }
        out.extend_from_slice(&self.audio_buffer);
        self.audio_buffer.clear();
    }

    pub fn take_frame_into(&mut self, buf: &mut Vec<u32>) -> bool {
        if !self.frame_ready {
            if Self::env_force_title_scene() || Self::env_force_title_now() {
                *buf = Self::synth_title_frame();
                return true;
            }
            return false;
        }
        self.frame_ready = false;
        if Self::env_force_title_now() || Self::env_force_title_scene() {
            *buf = Self::synth_title_frame();
            return true;
        }
        let w = self.current_display_width;
        let h = self.current_display_height;
        let x_off = *self.current_display_x_offset;
        let y_off = self.current_display_y_offset;
        let needed = w * h;
        buf.resize(needed, 0);
        for y in 0..h {
            let src_y = y + y_off;
            if src_y >= FRAME_HEIGHT {
                break;
            }
            let src = src_y * FRAME_WIDTH + x_off;
            let dst = y * w;
            buf[dst..dst + w].copy_from_slice(&self.framebuffer[src..src + w]);
        }
        true
    }

    pub fn take_frame(&mut self) -> Option<Vec<u32>> {
        if !self.frame_ready {
            if Self::env_force_title_scene() || Self::env_force_title_now() {
                return Some(Self::synth_title_frame());
            } else {
                return None;
            }
        }
        self.frame_ready = false;
        if Self::env_force_title_now() || Self::env_force_title_scene() {
            return Some(Self::synth_title_frame());
        }
        let w = self.current_display_width;
        let h = self.current_display_height;
        let x_off = *self.current_display_x_offset;
        let y_off = self.current_display_y_offset;
        let mut out = vec![0u32; w * h];
        for y in 0..h {
            let src_y = y + y_off;
            if src_y >= FRAME_HEIGHT {
                break;
            }
            let src = src_y * FRAME_WIDTH + x_off;
            let dst = y * w;
            out[dst..dst + w].copy_from_slice(&self.framebuffer[src..src + w]);
        }
        Some(out)
    }

    fn synth_title_frame() -> Vec<u32> {
        const W: usize = 256;
        const H: usize = FRAME_HEIGHT;
        let mut fb = vec![0u32; W * H];
        for y in 0..H {
            let band = (y / 30) as u32;
            let base = 0x101820 + (band * 0x030303);
            for x in 0..W {
                fb[y * W + x] = base;
            }
        }
        let text = b"KATO-CHAN & KEN-CHAN";
        let colors = [0xC8E4FF, 0x80B0FF, 0x4060E0, 0x102040];
        let mut draw_char = |ch: u8, ox: usize, oy: usize, col: u32| {
            for dy in 0..10 {
                for dx in 0..8 {
                    if (FONT[(ch as usize).wrapping_sub(32)].get(dy).unwrap_or(&0) >> (7 - dx)) & 1
                        == 1
                    {
                        let x = ox + dx;
                        let y = oy + dy;
                        if x < W && y < H {
                            fb[y * W + x] = col;
                        }
                    }
                }
            }
        };
        let start_x = 24;
        let start_y = 60;
        for (i, &ch) in text.iter().enumerate() {
            let col = colors[i % colors.len()];
            draw_char(ch, start_x + i * 9, start_y, col);
        }
        fb
    }

    pub(super) fn force_title_scene(&mut self) {
        let ctrl = VDC_CTRL_ENABLE_BACKGROUND_LEGACY
            | VDC_CTRL_ENABLE_SPRITES_LEGACY
            | VDC_CTRL_ENABLE_BACKGROUND
            | VDC_CTRL_ENABLE_SPRITES;
        self.vdc.registers[0x04] = ctrl;
        self.vdc.registers[0x05] = ctrl;
        self.vdc
            .raise_status(VDC_STATUS_DS | VDC_STATUS_DV | VDC_STATUS_VBL);
        self.vdc.registers[0x09] = 0x0010;
        for (i, slot) in self.vce.palette.iter_mut().enumerate() {
            *slot = ((i as u16 & 0x0F) << 8) | (((i as u16 >> 4) & 0x0F) << 4) | (i as u16 & 0x0F);
        }
        for tile in 0..0x200 {
            for row in 0..8 {
                let pattern = (((tile + row) & 1) * 0xFF) as u16;
                let addr = tile * 8 + row;
                if let Some(slot) = self.vdc.vram.get_mut(addr) {
                    *slot = pattern;
                }
            }
        }
        let (map_w, map_h) = self.vdc.map_dimensions();
        let base = self.vdc.map_base_address();
        let mask = self.vdc.vram.len() - 1;
        for y in 0..map_h {
            for x in 0..map_w {
                let idx = ((y * map_w + x) & 0x7FF) as u16;
                let addr = (base + ((y * map_w + x) % 0x400)) & mask;
                self.vdc.vram[addr] = idx;
            }
        }
        self.vdc.satb[0] = 0;
        self.vdc.satb[1] = 0;
        self.vdc.satb[2] = 0;
        self.vdc.satb[3] = 0;
        self.frame_ready = true;
    }

    pub fn framebuffer(&self) -> &[u32] {
        &self.framebuffer
    }

    pub fn display_width(&self) -> usize {
        self.current_display_width
    }

    pub(crate) fn compute_display_height(&self) -> (usize, usize) {
        let timing_programmed = self.vdc.registers[0x0D] != 0
            || self.vdc.registers[0x0E] != 0
            || (self.vdc.registers[0x0C] & 0xFF00) != 0;
        if !timing_programmed {
            return (DEFAULT_DISPLAY_HEIGHT, 0);
        }
        let mut first_active = FRAME_HEIGHT;
        let mut last_active = 0usize;
        for y in 0..FRAME_HEIGHT {
            if self.vdc.output_row_in_active_window(y) {
                if y < first_active {
                    first_active = y;
                }
                last_active = y;
            }
        }
        if first_active >= FRAME_HEIGHT {
            return (FRAME_HEIGHT, 0);
        }
        let active_count = last_active - first_active + 1;
        (active_count, first_active)
    }

    pub fn display_height(&self) -> usize {
        self.current_display_height
    }

    pub fn display_y_offset(&self) -> usize {
        self.current_display_y_offset
    }

    pub fn vdc_control_register(&self) -> u16 {
        self.vdc.control()
    }

    pub fn vdc_control_for_render(&self) -> u16 {
        self.vdc.control_for_render()
    }

    pub fn vdc_mawr(&self) -> u16 {
        self.vdc.mawr
    }

    pub fn vdc_satb_pending(&self) -> bool {
        self.vdc.satb_pending()
    }

    pub fn vdc_satb_written(&self) -> bool {
        self.vdc.satb_written
    }

    pub fn vdc_satb_source(&self) -> u16 {
        self.vdc.satb_source()
    }

    pub fn vdc_satb_nonzero_words(&self) -> usize {
        self.vdc.satb.iter().filter(|&&word| word != 0).count()
    }

    pub fn vdc_satb_word(&self, index: usize) -> u16 {
        self.vdc.satb.get(index).copied().unwrap_or(0)
    }

    pub fn vdc_dma_control(&self) -> u16 {
        self.vdc.dma_control
    }

    pub fn vdc_scroll_line(&self, line: usize) -> (u16, u16) {
        self.vdc.scroll_line(line)
    }

    pub fn vdc_scroll_line_valid(&self, line: usize) -> bool {
        self.vdc.scroll_line_valid(line)
    }

    pub fn vdc_scroll_line_y_offset(&self, line: usize) -> u16 {
        if line < self.vdc.scroll_line_y_offset.len() {
            self.vdc.scroll_line_y_offset[line]
        } else {
            0
        }
    }

    pub fn vdc_line_state_index_for_row(&self, row: usize) -> usize {
        self.vdc.line_state_index_for_frame_row(row)
    }

    pub fn vdc_zoom_line(&self, line: usize) -> (u16, u16) {
        self.vdc.zoom_line(line)
    }

    pub fn vdc_control_line(&self, line: usize) -> u16 {
        self.vdc.control_line(line)
    }

    pub fn vdc_vram(&self) -> &[u16] {
        &self.vdc.vram
    }

    pub fn vdc_map_entry_address(&self, tile_row: usize, tile_col: usize) -> usize {
        self.vdc.map_entry_address(tile_row, tile_col)
    }

    fn enqueue_audio_samples(&mut self, phi_cycles: u32) {
        self.audio_total_phi_cycles.0 = self
            .audio_total_phi_cycles
            .0
            .saturating_add(phi_cycles as u64);
        self.audio_phi_accumulator = self
            .audio_phi_accumulator
            .saturating_add(phi_cycles as u64 * AUDIO_SAMPLE_RATE as u64);
        while self.audio_phi_accumulator >= MASTER_CLOCK_HZ as u64 {
            self.audio_phi_accumulator -= MASTER_CLOCK_HZ as u64;
            let psg_cycles = self.psg_cycles_for_host_sample();
            let sample = self.psg.render_host_sample(psg_cycles);
            self.audio_buffer.push(sample);
            self.audio_total_generated_samples.0 =
                self.audio_total_generated_samples.0.saturating_add(1);
        }
    }

    fn psg_cycles_for_host_sample(&mut self) -> u32 {
        self.audio_psg_accumulator.0 = self
            .audio_psg_accumulator
            .0
            .saturating_add(PSG_CLOCK_HZ as u64);
        let psg_cycles = (self.audio_psg_accumulator.0 / AUDIO_SAMPLE_RATE as u64) as u32;
        self.audio_psg_accumulator.0 %= AUDIO_SAMPLE_RATE as u64;
        psg_cycles
    }

    pub fn irq_pending(&self) -> bool {
        (self.interrupt_request & self.enabled_irq_mask()) != 0
    }

    pub fn pending_interrupts(&self) -> u8 {
        self.interrupt_request & self.enabled_irq_mask()
    }

    pub fn raise_irq(&mut self, mask: u8) {
        self.interrupt_request |= mask;
    }

    pub fn clear_irq(&mut self, mask: u8) {
        self.interrupt_request &= !mask;
    }

    pub fn acknowledge_irq(&mut self, mask: u8) {
        self.clear_irq(mask);
        if mask & IRQ_REQUEST_IRQ2 != 0 {
            self.psg.acknowledge();
        }
    }

    pub fn next_irq(&self) -> Option<u8> {
        let masked = self.pending_interrupts();
        if masked & IRQ_REQUEST_TIMER != 0 {
            return Some(IRQ_REQUEST_TIMER);
        }
        if masked & IRQ_REQUEST_IRQ1 != 0 {
            return Some(IRQ_REQUEST_IRQ1);
        }
        if masked & IRQ_REQUEST_IRQ2 != 0 {
            return Some(IRQ_REQUEST_IRQ2);
        }
        None
    }

    #[cfg(feature = "trace_hw_writes")]
    fn cpu_pc_for_trace(&self) -> u16 {
        self.last_pc_for_trace.unwrap_or(0)
    }

    pub(super) fn refresh_vdc_irq(&mut self) {
        #[cfg(debug_assertions)]
        {
            const FORCE_AFTER_WRITES: u64 = 5_000;
            if *self.debug_force_ds_after >= FORCE_AFTER_WRITES {
                self.vdc.raise_status(VDC_STATUS_DS | VDC_STATUS_DV);
            }
        }
        if Self::env_force_vdc_dsdv() {
            self.vdc.raise_status(VDC_STATUS_DS | VDC_STATUS_DV);
        }
        if Self::env_force_irq1() {
            self.interrupt_request |= IRQ_REQUEST_IRQ1;
        }
        if Self::env_force_irq2() {
            self.interrupt_request |= IRQ_REQUEST_IRQ2;
        }
        if self.vdc.irq_active() {
            self.interrupt_request |= IRQ_REQUEST_IRQ1;
        } else {
            self.interrupt_request &= !IRQ_REQUEST_IRQ1;
        }
    }

    fn perform_cram_dma(&mut self) {
        let raw_length = self.vdc.registers[0x12];
        let mut words = raw_length as usize;
        if words == 0 {
            words = 0x200;
        }
        words = words.min(0x200);

        let mut src = self.vdc.marr & 0x7FFF;
        let mut index = self.vce.address_index();

        for _ in 0..words {
            let word = *self.vdc.vram.get(src as usize).unwrap_or(&0);
            if let Some(slot) = self.vce.palette.get_mut(index) {
                *slot = word;
            }
            index = (index + 1) & 0x01FF;
            src = Vdc::advance_vram_addr(src, false);
        }

        self.vdc.marr = src & 0x7FFF;
        self.vdc.registers[0x01] = self.vdc.marr;
        self.vce.set_address(index as u16);
        let busy_cycles = (words as u32).saturating_mul(VDC_DMA_WORD_CYCLES);
        self.vdc.set_busy(busy_cycles);
        self.vdc.raise_status(VDC_STATUS_DV);
        self.vdc.cram_pending = false;
    }

    fn perform_vram_dma(&mut self) {
        #[cfg(any(debug_assertions, feature = "trace_hw_writes"))]
        eprintln!(
            "  VDC VRAM DMA start ctrl={:04X} src={:04X} dst={:04X} len={:04X}",
            self.vdc.dma_control,
            self.vdc.dma_source,
            self.vdc.dma_destination,
            self.vdc.registers[0x12]
        );
        let original_len = self.vdc.registers[0x12];
        let words = original_len as u32 + 1;

        let src_dec = self.vdc.dma_control & DMA_CTRL_SRC_DEC != 0;
        let dst_dec = self.vdc.dma_control & DMA_CTRL_DST_DEC != 0;

        let mut src = self.vdc.dma_source & 0x7FFF;
        let mut dst = self.vdc.dma_destination & 0x7FFF;

        for _ in 0..words {
            let value = self.vdc.vram[(src as usize) & 0x7FFF];
            self.vdc.write_vram_dma_word(dst, value);

            src = Vdc::advance_vram_addr(src, src_dec);
            dst = Vdc::advance_vram_addr(dst, dst_dec);
        }

        self.vdc.dma_source = src;
        self.vdc.dma_destination = dst;
        self.vdc.registers[0x10] = self.vdc.dma_source;
        self.vdc.registers[0x11] = self.vdc.dma_destination;
        self.vdc.registers[0x12] = 0xFFFF;

        #[cfg(any(debug_assertions, feature = "trace_hw_writes"))]
        eprintln!(
            "  VDC VRAM DMA end src={:04X} dst={:04X} len={:04X}",
            self.vdc.dma_source, self.vdc.dma_destination, original_len
        );

        let busy_cycles = words.saturating_mul(VDC_DMA_WORD_CYCLES);
        self.vdc.set_busy(busy_cycles);
        self.vdc.raise_status(VDC_STATUS_DV);

        if Self::env_force_cram_from_vram() {
            for i in 0..0x200 {
                let word = self.vdc.vram.get(i).copied().unwrap_or(0);
                if let Some(slot) = self.vce.palette.get_mut(i) {
                    *slot = word;
                }
            }
            #[cfg(any(debug_assertions, feature = "trace_hw_writes"))]
            eprintln!("  DEBUG PCE_FORCE_CRAM_FROM_VRAM applied (first 512 words)");
        }
    }

    fn enabled_irq_mask(&self) -> u8 {
        let mut mask = 0;
        if self.interrupt_disable & IRQ_DISABLE_IRQ2 == 0 {
            mask |= IRQ_REQUEST_IRQ2;
        }
        if self.interrupt_disable & IRQ_DISABLE_IRQ1 == 0 {
            mask |= IRQ_REQUEST_IRQ1;
        }
        if self.interrupt_disable & IRQ_DISABLE_TIMER == 0 {
            mask |= IRQ_REQUEST_TIMER;
        }
        mask
    }
}
