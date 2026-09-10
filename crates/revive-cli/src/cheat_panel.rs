use egui::{self, RichText};
use revive_cheat::CheatManager;
use revive_core::CoreInstance;

#[path = "cheat_panel/hex_viewer.rs"]
mod hex_viewer;
#[path = "cheat_panel/memory.rs"]
mod memory;
#[path = "cheat_panel/search.rs"]
mod search;

use hex_viewer::HexViewerState;
use memory::MemorySnapshot;
pub use memory::MemoryWrite;
use search::CheatSearchUi;

pub struct CheatPanel {
    active_tab: ActiveTab,
    hex_viewer: HexViewerState,
    search_ui: CheatSearchUi,
    visible: bool,
    ram_snapshot: MemorySnapshot,
    previous_snapshot: MemorySnapshot,
    search_snapshot: MemorySnapshot,
    memory_dirty: bool,
    refresh_requested: bool,
    paused: bool,
    auto_refresh: bool,
}

impl CheatPanel {
    pub fn new() -> Self {
        Self {
            active_tab: ActiveTab::HexViewer,
            hex_viewer: HexViewerState::new(),
            search_ui: CheatSearchUi::new(),
            visible: false,
            ram_snapshot: MemorySnapshot::default(),
            previous_snapshot: MemorySnapshot::default(),
            search_snapshot: MemorySnapshot::default(),
            memory_dirty: true,
            refresh_requested: false,
            paused: false,
            auto_refresh: true,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn is_paused(&self) -> bool {
        self.visible && self.paused
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            self.refresh_requested = true;
            self.invalidate_memory();
        }
    }

    pub fn invalidate_memory(&mut self) {
        self.memory_dirty = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn show_panel(
        &mut self,
        ui: &mut egui::Ui,
        core: &CoreInstance,
        cheats: &mut CheatManager,
        cheat_path: Option<&std::path::Path>,
    ) -> Vec<MemoryWrite> {
        let mut writes = Vec::new();
        let previous_tab = self.active_tab;
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.active_tab, ActiveTab::HexViewer, "Hex Viewer");
            ui.selectable_value(&mut self.active_tab, ActiveTab::CheatSearch, "Cheat Search");
            ui.separator();
            ui.checkbox(&mut self.paused, "Pause");
        });
        if previous_tab != self.active_tab {
            self.memory_dirty = true;
        }
        ui.separator();

        match self.active_tab {
            ActiveTab::HexViewer => {
                ui.horizontal(|ui| {
                    if ui.button("Refresh").clicked() {
                        self.refresh_requested = true;
                    }
                    if ui.checkbox(&mut self.auto_refresh, "Auto").changed() && self.auto_refresh {
                        self.refresh_requested = true;
                    }
                });
                ui.separator();
                if self.refresh_requested
                    || self.ram_snapshot.is_empty()
                    || (self.auto_refresh && self.memory_dirty)
                {
                    std::mem::swap(&mut self.ram_snapshot, &mut self.previous_snapshot);
                    self.ram_snapshot.capture_into(core);
                    self.refresh_requested = false;
                    self.memory_dirty = false;
                }
                ui.label(RichText::new(self.ram_snapshot.region_summary()).small());
                self.hex_viewer.show(
                    ui,
                    &self.ram_snapshot,
                    self.previous_snapshot.bytes(),
                    &mut writes,
                );
            }
            ActiveTab::CheatSearch => {
                if self.memory_dirty || self.search_snapshot.is_empty() || self.refresh_requested {
                    self.search_snapshot.capture_into(core);
                    self.memory_dirty = false;
                    self.refresh_requested = false;
                }
                ui.label(RichText::new(self.search_snapshot.region_summary()).small());
                self.search_ui
                    .show(ui, &self.search_snapshot, cheats, cheat_path);
            }
        }

        writes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActiveTab {
    HexViewer,
    CheatSearch,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(deprecated)]
    fn draw(panel: &mut CheatPanel, core: &CoreInstance) {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                panel.show_panel(ui, core, &mut CheatManager::new(), None);
            });
        });
    }

    #[test]
    fn snapshots_reuse_buffers_respect_manual_refresh_and_only_update_active_tab() {
        let mut core = crate::test_core();
        let mut panel = CheatPanel::new();
        panel.toggle();
        core.write_memory_byte("wram", 0, 10);
        draw(&mut panel, &core);
        assert_eq!(panel.ram_snapshot.bytes()[0], 10);
        let first_buffer = panel.ram_snapshot.bytes().as_ptr();

        core.write_memory_byte("wram", 0, 20);
        panel.invalidate_memory();
        draw(&mut panel, &core);
        assert_eq!(panel.previous_snapshot.bytes()[0], 10);
        assert_eq!(panel.ram_snapshot.bytes()[0], 20);
        core.write_memory_byte("wram", 0, 30);
        panel.invalidate_memory();
        draw(&mut panel, &core);
        assert_eq!(panel.ram_snapshot.bytes().as_ptr(), first_buffer);
        assert_eq!(panel.previous_snapshot.bytes()[0], 20);

        panel.auto_refresh = false;
        core.write_memory_byte("wram", 0, 40);
        panel.invalidate_memory();
        draw(&mut panel, &core);
        assert_eq!(panel.ram_snapshot.bytes()[0], 30);
        panel.refresh_requested = true;
        draw(&mut panel, &core);
        assert_eq!(panel.ram_snapshot.bytes()[0], 40);
        assert_eq!(panel.previous_snapshot.bytes()[0], 30);

        // A paused frame with no mutations retains both buffers and the diff.
        panel.auto_refresh = true;
        panel.paused = true;
        let buffer = panel.ram_snapshot.bytes().as_ptr();
        draw(&mut panel, &core);
        assert_eq!(panel.ram_snapshot.bytes().as_ptr(), buffer);
        assert_eq!(panel.previous_snapshot.bytes()[0], 30);

        panel.active_tab = ActiveTab::CheatSearch;
        panel.invalidate_memory();
        core.write_memory_byte("wram", 0, 50);
        draw(&mut panel, &core);
        assert_eq!(panel.search_snapshot.bytes()[0], 50);
        assert_eq!(panel.ram_snapshot.bytes()[0], 40);
    }
}
