use egui::{self, Color32, RichText};
use revive_cheat::{parse_u8_value, CheatManager, CheatSearch, SearchFilter};

use super::memory::MemorySnapshot;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FilterKind {
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    Increased,
    Decreased,
    Changed,
    Unchanged,
    IncreasedBy,
    DecreasedBy,
}

impl FilterKind {
    const ALL: [FilterKind; 10] = [
        Self::Equal,
        Self::NotEqual,
        Self::GreaterThan,
        Self::LessThan,
        Self::Increased,
        Self::Decreased,
        Self::Changed,
        Self::Unchanged,
        Self::IncreasedBy,
        Self::DecreasedBy,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Equal => "Equal to",
            Self::NotEqual => "Not equal to",
            Self::GreaterThan => "Greater than",
            Self::LessThan => "Less than",
            Self::Increased => "Increased",
            Self::Decreased => "Decreased",
            Self::Changed => "Changed",
            Self::Unchanged => "Unchanged",
            Self::IncreasedBy => "Increased by",
            Self::DecreasedBy => "Decreased by",
        }
    }

    fn needs_value(self) -> bool {
        matches!(
            self,
            Self::Equal
                | Self::NotEqual
                | Self::GreaterThan
                | Self::LessThan
                | Self::IncreasedBy
                | Self::DecreasedBy
        )
    }
}

pub(crate) struct CheatSearchUi {
    search: CheatSearch,
    filter_kind: FilterKind,
    filter_value: String,
    new_cheat_addr: String,
    new_cheat_value: String,
}

impl CheatSearchUi {
    pub(crate) fn new() -> Self {
        Self {
            search: CheatSearch::new(0),
            filter_kind: FilterKind::Equal,
            filter_value: String::new(),
            new_cheat_addr: String::new(),
            new_cheat_value: String::new(),
        }
    }

    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        live: &MemorySnapshot,
        cheats: &mut CheatManager,
        cheat_path: Option<&std::path::Path>,
    ) {
        self.search.resize(live.len());

        ui.horizontal(|ui| {
            ui.heading("Cheat Search");
            ui.separator();
            ui.label(RichText::new(format!("{} bytes", live.len())).small());
        });
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Snapshot").clicked() {
                self.search.snapshot(live.bytes());
            }
            if ui.button("Reset").clicked() {
                self.search.reset();
            }
            ui.label(format!("Candidates: {}", self.search.candidate_count()));
            if self.search.has_snapshot() {
                ui.label(
                    RichText::new("(snapshot taken)").color(Color32::from_rgb(0x44, 0xCC, 0x44)),
                );
            }
        });

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Filter:");
            egui::ComboBox::from_id_salt("revive_filter_kind")
                .selected_text(self.filter_kind.label())
                .width(130.0)
                .show_ui(ui, |ui| {
                    for kind in FilterKind::ALL {
                        ui.selectable_value(&mut self.filter_kind, kind, kind.label());
                    }
                });
            if self.filter_kind.needs_value() {
                ui.label("Value:");
                ui.add(egui::TextEdit::singleline(&mut self.filter_value).desired_width(50.0));
            }
            if ui.button("Apply").clicked() {
                if let Some(filter) = self.build_filter() {
                    self.search.apply_filter(filter, live.bytes());
                }
            }
        });

        ui.separator();
        let candidates = self.search.candidates();
        let snap = self.search.previous_snapshot();
        ui.label(format!("Results: {}", candidates.len()));
        ui.horizontal(|ui| {
            ui.style_mut().override_font_id = Some(egui::FontId::monospace(12.0));
            ui.label("Addr");
            ui.label("Prev");
            ui.label("Cur");
            ui.label("");
        });

        let row_height = (ui.text_style_height(&egui::TextStyle::Monospace) + 4.0).max(16.0);
        egui::ScrollArea::vertical()
            .id_salt("revive_cheat_results")
            .max_height(150.0)
            .show_rows(ui, row_height, candidates.len(), |ui, row_range| {
                ui.style_mut().override_font_id = Some(egui::FontId::monospace(12.0));
                for row_idx in row_range {
                    let Some(&addr) = candidates.get(row_idx) else {
                        continue;
                    };
                    let offset = addr as usize;
                    let current = live.bytes().get(offset).copied().unwrap_or(0);
                    let previous = snap.map(|snapshot| snapshot.get(addr)).unwrap_or(0);
                    ui.horizontal(|ui| {
                        ui.label(live.format_combined_addr(offset));
                        ui.label(format!("{previous:02X}"));
                        ui.label(format!("{current:02X}"));
                        if ui.small_button("Add").clicked() {
                            if let Some(write) = live.write_for_combined_offset(offset, current) {
                                cheats.add(
                                    write.region,
                                    write.offset as u32,
                                    write.value,
                                    live.format_combined_addr(offset),
                                );
                            }
                        }
                    });
                }
            });

        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("Active Cheats");
            ui.separator();
            if let Some(path) = cheat_path {
                if ui.button("Save").clicked() {
                    match cheats.save_to_file(path) {
                        Ok(()) => eprintln!(
                            "Saved {} cheats to {}",
                            cheats.entries.len(),
                            path.display()
                        ),
                        Err(err) => eprintln!("Failed to save cheats: {err}"),
                    }
                }
                if path.exists() && ui.button("Load").clicked() {
                    match CheatManager::load_from_file(path) {
                        Ok(loaded) => {
                            *cheats = loaded;
                            eprintln!(
                                "Loaded {} cheats from {}",
                                cheats.entries.len(),
                                path.display()
                            );
                        }
                        Err(err) => eprintln!("Failed to load cheats: {err}"),
                    }
                }
            } else {
                ui.label(RichText::new("No --cheats path; Save unavailable").small());
            }
        });

        let mut remove_idx = None;
        egui::ScrollArea::vertical()
            .id_salt("revive_cheat_entries")
            .max_height(120.0)
            .show(ui, |ui| {
                ui.style_mut().override_font_id = Some(egui::FontId::monospace(12.0));
                for (index, entry) in cheats.entries.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut entry.enabled, "");
                        let combined =
                            live.combined_offset_for_region(&entry.region, entry.offset as usize);
                        ui.label(
                            combined
                                .map(|offset| live.format_combined_addr(offset))
                                .unwrap_or_else(|| {
                                    format!("{}:{:04X}", entry.region, entry.offset)
                                }),
                        );
                        ui.label("=");
                        let mut value = format!("{:02X}", entry.value);
                        let resp =
                            ui.add(egui::TextEdit::singleline(&mut value).desired_width(30.0));
                        if resp.changed() {
                            if let Some(parsed) = parse_u8_value(&value) {
                                entry.value = parsed;
                            }
                        }
                        ui.text_edit_singleline(&mut entry.label);
                        if ui.small_button("X").clicked() {
                            remove_idx = Some(index);
                        }
                    });
                }
            });
        if let Some(index) = remove_idx {
            cheats.remove(index);
        }

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Add:");
            ui.add(
                egui::TextEdit::singleline(&mut self.new_cheat_addr)
                    .desired_width(85.0)
                    .hint_text("wram:1234"),
            );
            ui.label("=");
            ui.add(
                egui::TextEdit::singleline(&mut self.new_cheat_value)
                    .desired_width(30.0)
                    .hint_text("xx"),
            );
            if ui.button("Add").clicked() {
                if let (Some(offset), Some(value)) = (
                    live.parse_addr(&self.new_cheat_addr),
                    parse_u8_value(&self.new_cheat_value),
                ) {
                    if let Some(write) = live.write_for_combined_offset(offset, value) {
                        cheats.add(
                            write.region,
                            write.offset as u32,
                            write.value,
                            live.format_combined_addr(offset),
                        );
                        self.new_cheat_addr.clear();
                        self.new_cheat_value.clear();
                    }
                }
            }
        });
    }

    fn build_filter(&self) -> Option<SearchFilter> {
        let parse_value = || parse_u8_value(&self.filter_value);
        match self.filter_kind {
            FilterKind::Equal => parse_value().map(SearchFilter::Equal),
            FilterKind::NotEqual => parse_value().map(SearchFilter::NotEqual),
            FilterKind::GreaterThan => parse_value().map(SearchFilter::GreaterThan),
            FilterKind::LessThan => parse_value().map(SearchFilter::LessThan),
            FilterKind::Increased => Some(SearchFilter::Increased),
            FilterKind::Decreased => Some(SearchFilter::Decreased),
            FilterKind::Changed => Some(SearchFilter::Changed),
            FilterKind::Unchanged => Some(SearchFilter::Unchanged),
            FilterKind::IncreasedBy => parse_value().map(SearchFilter::IncreasedBy),
            FilterKind::DecreasedBy => parse_value().map(SearchFilter::DecreasedBy),
        }
    }
}
