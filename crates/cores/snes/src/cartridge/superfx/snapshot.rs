use super::*;

impl SuperFx {
    pub(super) fn latest_stop_snapshot_matches_filters(&self) -> bool {
        let pbr_ok =
            superfx_screen_buffer_stop_pbr_filter().is_none_or(|pbr| pbr == self.latest_stop_pbr);
        let pc_ok =
            superfx_screen_buffer_stop_pc_filter().is_none_or(|pc| pc == self.latest_stop_pc);
        pbr_ok && pc_ok
    }

    pub(super) fn selected_screen_snapshot(&self) -> Option<(&[u8], u8, u16, u8, u8)> {
        let filter_pbr = superfx_screen_buffer_stop_pbr_filter();
        let filter_pc = superfx_screen_buffer_stop_pc_filter();
        if filter_pbr.is_some() || filter_pc.is_some() {
            if let Some(snapshot) = self.recent_stop_snapshots.iter().rev().find(|snapshot| {
                filter_pbr.is_none_or(|pbr| pbr == snapshot.pbr)
                    && filter_pc.is_none_or(|pc| pc == snapshot.pc)
            }) {
                return Some((
                    snapshot.data.as_slice(),
                    snapshot.scbr,
                    snapshot.height,
                    snapshot.bpp,
                    snapshot.mode,
                ));
            }
        }
        if let Some(snapshot) = self.debug_pc_snapshot.as_ref() {
            return Some((
                snapshot.data.as_slice(),
                snapshot.scbr,
                snapshot.height,
                snapshot.bpp,
                snapshot.mode,
            ));
        }
        if self.latest_stop_snapshot_valid && self.latest_stop_snapshot_matches_filters() {
            return Some((
                self.latest_stop_snapshot.as_slice(),
                self.latest_stop_scbr,
                self.latest_stop_height,
                self.latest_stop_bpp,
                self.latest_stop_mode,
            ));
        }
        None
    }

    pub(super) fn display_screen_snapshot(&self) -> Option<(&[u8], u8, u16, u8, u8)> {
        self.display_snapshot_valid.then_some((
            self.display_snapshot.as_slice(),
            self.display_snapshot_scbr,
            self.display_snapshot_height,
            self.display_snapshot_bpp,
            self.display_snapshot_mode,
        ))
    }

    pub fn capture_display_snapshot_for_dma(&mut self, addr: usize, len: usize) -> bool {
        if len == 0 || self.game_ram.is_empty() {
            return false;
        }

        let selected = self
            .selected_screen_snapshot()
            .map(|(snapshot, scbr, height, bpp, mode)| (snapshot.to_vec(), scbr, height, bpp, mode))
            .or_else(|| {
                let len = self.screen_buffer_len()?;
                let start = self.screen_base_addr();
                let end = start.checked_add(len)?.min(self.game_ram.len());
                let height = self.effective_screen_height()? as u16;
                let bpp = self.bits_per_pixel()? as u8;
                let mode = self.effective_screen_layout_mode();
                (start < end).then(|| {
                    (
                        self.game_ram[start..end].to_vec(),
                        self.scbr,
                        height,
                        bpp,
                        mode,
                    )
                })
            });

        let Some((snapshot, scbr, height, bpp, mode)) = selected else {
            return false;
        };
        if snapshot.is_empty() {
            return false;
        }

        let dma_start = addr % self.game_ram.len();
        let dma_end = dma_start.saturating_add(len);
        let snapshot_start = (scbr as usize) << 10;
        let snapshot_end = snapshot_start.saturating_add(snapshot.len());
        if dma_start >= snapshot_end || dma_end <= snapshot_start {
            return false;
        }

        let metadata_changed = !self.display_snapshot_valid
            || self.display_snapshot_scbr != scbr
            || self.display_snapshot_height != height
            || self.display_snapshot_bpp != bpp
            || self.display_snapshot_mode != mode
            || self.display_snapshot.len() != snapshot.len();
        if metadata_changed {
            self.display_snapshot = vec![0; snapshot.len()];
        }

        let copy_start = dma_start.max(snapshot_start);
        let copy_end = dma_end.min(snapshot_end);
        let copy_len = copy_end.saturating_sub(copy_start);
        if copy_len == 0 {
            return false;
        }
        let rel = copy_start - snapshot_start;
        self.display_snapshot[rel..rel + copy_len].copy_from_slice(&snapshot[rel..rel + copy_len]);
        self.display_snapshot_valid = true;
        self.display_snapshot_scbr = scbr;
        self.display_snapshot_height = height;
        self.display_snapshot_bpp = bpp;
        self.display_snapshot_mode = mode;
        if trace_superfx_display_captures_enabled() {
            let nonzero = self
                .display_snapshot
                .iter()
                .filter(|&&byte| byte != 0)
                .count();
            eprintln!(
                "[SFX-DISPLAY-CAPTURE] frame={} dma={:05X}+{} copy={:05X}+{} scbr={:02X} h={} bpp={} mode={} len={} nonzero={}",
                current_trace_superfx_frame(),
                dma_start,
                len,
                copy_start,
                copy_len,
                scbr,
                height,
                bpp,
                mode,
                self.display_snapshot.len(),
                nonzero
            );
        }
        true
    }

    pub(super) fn maybe_capture_debug_screen_snapshot(&mut self, pc: u16) {
        let Some(filter_pc) = superfx_screen_buffer_capture_pc_filter() else {
            return;
        };
        if pc != filter_pc {
            return;
        }
        if superfx_screen_buffer_capture_pbr_filter().is_some_and(|pbr| pbr != self.pbr) {
            return;
        }
        let Some(len) = self.screen_buffer_len() else {
            return;
        };
        let Some(height) = self.effective_screen_height() else {
            return;
        };
        let Some(bpp) = self.bits_per_pixel() else {
            return;
        };
        let start = self.screen_base_addr();
        let end = start.saturating_add(len).min(self.game_ram.len());
        if start >= end {
            return;
        }
        self.debug_pc_snapshot = Some(StopSnapshot {
            data: self.game_ram[start..end].to_vec(),
            scbr: self.scbr,
            height: height as u16,
            bpp: bpp as u8,
            mode: self.scmr & 0x03,
            pc,
            pbr: self.pbr,
        });
    }

    pub(super) fn selected_tile_snapshot(&self) -> Option<(&[u8], u16, u8, u8)> {
        if let Some(pc) = superfx_tile_snapshot_pc_filter() {
            let rev_index = superfx_tile_snapshot_rev_index();
            if let Some(snapshot) = self
                .recent_tile_snapshots
                .iter()
                .rev()
                .filter(|snapshot| snapshot.pc == pc)
                .nth(rev_index)
            {
                return Some((
                    snapshot.data.as_slice(),
                    snapshot.height,
                    snapshot.bpp,
                    snapshot.mode,
                ));
            }
        }
        if self.tile_snapshot_valid {
            return Some((
                self.tile_snapshot.as_slice(),
                self.tile_snapshot_height,
                self.tile_snapshot_bpp,
                self.tile_snapshot_mode,
            ));
        }
        None
    }
}
