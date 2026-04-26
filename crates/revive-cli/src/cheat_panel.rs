use egui::{self, RichText};
use revive_cheat::CheatManager;

#[path = "cheat_panel/hex_viewer.rs"]
mod hex_viewer;
#[path = "cheat_panel/memory.rs"]
mod memory;
#[path = "cheat_panel/search.rs"]
mod search;

use hex_viewer::HexViewerState;
pub use memory::{MemorySnapshot, MemoryWrite};
use search::CheatSearchUi;

pub struct CheatPanel {
    active_tab: ActiveTab,
    hex_viewer: HexViewerState,
    search_ui: CheatSearchUi,
    visible: bool,
    ram_snapshot: MemorySnapshot,
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

    pub fn toggle(&mut self, live: &MemorySnapshot) {
        self.visible = !self.visible;
        if self.visible {
            self.refresh(live);
        }
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn show_panel(
        &mut self,
        ui: &mut egui::Ui,
        live: &MemorySnapshot,
        cheats: &mut CheatManager,
        cheat_path: Option<&std::path::Path>,
    ) -> Vec<MemoryWrite> {
        if self.auto_refresh || self.ram_snapshot.is_empty() {
            self.refresh(live);
        }

        let mut writes = Vec::new();
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.active_tab, ActiveTab::HexViewer, "Hex Viewer");
            ui.selectable_value(&mut self.active_tab, ActiveTab::CheatSearch, "Cheat Search");
            ui.separator();
            ui.checkbox(&mut self.paused, "Pause");
        });
        ui.label(RichText::new(live.region_summary()).small());
        ui.separator();

        match self.active_tab {
            ActiveTab::HexViewer => {
                ui.horizontal(|ui| {
                    if ui.button("Refresh").clicked() {
                        self.refresh_requested = true;
                    }
                    ui.checkbox(&mut self.auto_refresh, "Auto");
                });
                ui.separator();
                if self.refresh_requested {
                    self.refresh(live);
                    self.refresh_requested = false;
                }
                self.hex_viewer.show(ui, &self.ram_snapshot, &mut writes);
            }
            ActiveTab::CheatSearch => {
                self.search_ui.show(ui, live, cheats, cheat_path);
            }
        }

        writes
    }

    fn refresh(&mut self, live: &MemorySnapshot) {
        let prev = self.ram_snapshot.bytes().to_vec();
        self.ram_snapshot = live.clone();
        self.hex_viewer.update_prev(&prev);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActiveTab {
    HexViewer,
    CheatSearch,
}
