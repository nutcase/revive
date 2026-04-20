use egui::text::LayoutJob;
use egui::{self, Color32, FontId, RichText};
use revive_cheat::{parse_u8_value, CheatManager, CheatSearch, SearchFilter};
use revive_core::{CoreInstance, MemoryRegion};

const BYTES_PER_ROW: usize = 16;
const COLOR_ADDR: Color32 = Color32::from_rgb(0x88, 0x88, 0x88);
const COLOR_NORMAL: Color32 = Color32::from_rgb(0xCC, 0xCC, 0xCC);
const COLOR_CHANGED: Color32 = Color32::from_rgb(0xFF, 0x44, 0x44);
const COLOR_ASCII: Color32 = Color32::from_rgb(0x88, 0xAA, 0x88);

#[derive(Debug, Clone)]
pub struct MemoryWrite {
    pub region: String,
    pub offset: usize,
    pub value: u8,
}

#[derive(Debug, Clone, Default)]
pub struct MemorySnapshot {
    segments: Vec<MemorySegment>,
    data: Vec<u8>,
}

#[derive(Debug, Clone)]
struct MemorySegment {
    id: String,
    label: String,
    start: usize,
    len: usize,
    writable: bool,
}

impl MemorySnapshot {
    pub fn capture(core: &CoreInstance) -> Self {
        let mut segments = Vec::new();
        let mut data = Vec::new();
        for region in core.memory_regions() {
            let Some(bytes) = core.read_memory(region.id) else {
                continue;
            };
            let start = data.len();
            data.extend_from_slice(bytes);
            segments.push(segment_from_region(region, start, bytes.len()));
        }
        Self { segments, data }
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn bytes(&self) -> &[u8] {
        &self.data
    }

    fn segment_for_combined_offset(&self, offset: usize) -> Option<(&MemorySegment, usize)> {
        self.segments.iter().find_map(|segment| {
            let local = offset.checked_sub(segment.start)?;
            (local < segment.len).then_some((segment, local))
        })
    }

    fn write_for_combined_offset(&self, offset: usize, value: u8) -> Option<MemoryWrite> {
        let (segment, local) = self.segment_for_combined_offset(offset)?;
        segment.writable.then(|| MemoryWrite {
            region: segment.id.clone(),
            offset: local,
            value,
        })
    }

    fn combined_offset_for_region(&self, region: &str, offset: usize) -> Option<usize> {
        self.segments
            .iter()
            .find(|segment| segment.id == region && offset < segment.len)
            .map(|segment| segment.start + offset)
    }

    fn format_combined_addr(&self, offset: usize) -> String {
        let Some((segment, local)) = self.segment_for_combined_offset(offset) else {
            return format!("{offset:06X}");
        };
        format_segment_addr(segment, local)
    }

    fn parse_addr(&self, input: &str) -> Option<usize> {
        let input = input.trim();
        if input.is_empty() {
            return None;
        }

        if let Some((prefix, rest)) = input.split_once(':') {
            let rest = parse_hex(rest)? as usize;
            if prefix.eq_ignore_ascii_case("s") {
                return self.combined_offset_for_region("sram", rest);
            }
            if prefix.eq_ignore_ascii_case("7e") || prefix.eq_ignore_ascii_case("7f") {
                let bank = u8::from_str_radix(prefix, 16).ok()?;
                let wram_offset = ((usize::from(bank) - 0x7E) << 16) + rest;
                return self.combined_offset_for_region("wram", wram_offset);
            }
            return self.combined_offset_for_region(prefix, rest);
        }

        let offset = parse_hex(input)? as usize;
        (offset < self.len()).then_some(offset)
    }

    fn region_summary(&self) -> String {
        if self.segments.is_empty() {
            return "No readable memory regions".to_string();
        }
        self.segments
            .iter()
            .map(|segment| {
                let access = if segment.writable { "RW" } else { "RO" };
                format!(
                    "{} ({}) {} {} bytes",
                    segment.id, segment.label, access, segment.len
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

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
        let prev = self.ram_snapshot.data.clone();
        self.ram_snapshot = live.clone();
        self.hex_viewer.update_prev(&prev);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActiveTab {
    HexViewer,
    CheatSearch,
}

#[derive(Debug)]
struct HexViewerState {
    prev_ram: Vec<u8>,
    goto_addr: String,
    scroll_to_row: Option<usize>,
    edit_addr: String,
    edit_val: String,
}

impl HexViewerState {
    fn new() -> Self {
        Self {
            prev_ram: Vec::new(),
            goto_addr: String::new(),
            scroll_to_row: None,
            edit_addr: String::new(),
            edit_val: String::new(),
        }
    }

    fn update_prev(&mut self, previous: &[u8]) {
        self.prev_ram.clear();
        self.prev_ram.extend_from_slice(previous);
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &MemorySnapshot,
        writes: &mut Vec<MemoryWrite>,
    ) {
        let total_rows = snapshot.len().div_ceil(BYTES_PER_ROW);
        let mono = FontId::monospace(12.0);

        ui.horizontal(|ui| {
            ui.label("Go to:");
            let goto_resp =
                ui.add(egui::TextEdit::singleline(&mut self.goto_addr).desired_width(70.0));
            if (goto_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                || ui.button("Go").clicked()
            {
                if let Some(addr) = snapshot.parse_addr(&self.goto_addr) {
                    self.scroll_to_row = Some(addr / BYTES_PER_ROW);
                }
            }

            ui.separator();
            ui.label("Edit:");
            ui.add(egui::TextEdit::singleline(&mut self.edit_addr).desired_width(75.0));
            ui.label("=");
            let val_resp =
                ui.add(egui::TextEdit::singleline(&mut self.edit_val).desired_width(32.0));
            if (val_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                || ui.button("Set").clicked()
            {
                if let (Some(addr), Some(value)) = (
                    snapshot.parse_addr(&self.edit_addr),
                    parse_u8_value(&self.edit_val),
                ) {
                    if let Some(write) = snapshot.write_for_combined_offset(addr, value) {
                        writes.push(write);
                    }
                }
            }
        });
        ui.separator();

        if snapshot.is_empty() {
            ui.label(RichText::new("No readable memory regions").color(Color32::YELLOW));
            return;
        }

        let row_height = 16.0;
        let mut scroll_area = egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(ui.available_height());
        if let Some(row) = self.scroll_to_row.take() {
            scroll_area = scroll_area.vertical_scroll_offset(row as f32 * row_height);
        }

        scroll_area.show_rows(ui, row_height, total_rows, |ui, row_range| {
            for row_idx in row_range {
                let base = row_idx * BYTES_PER_ROW;
                let mut job = LayoutJob::default();
                append_text(
                    &mut job,
                    &format!("{} ", snapshot.format_combined_addr(base)),
                    &mono,
                    COLOR_ADDR,
                );

                for col in 0..BYTES_PER_ROW {
                    let addr = base + col;
                    if addr >= snapshot.len() {
                        append_text(&mut job, "   ", &mono, COLOR_NORMAL);
                        continue;
                    }
                    let byte = snapshot.bytes()[addr];
                    let changed = self.prev_ram.get(addr).copied() != Some(byte);
                    append_text(
                        &mut job,
                        &format!("{byte:02X} "),
                        &mono,
                        if changed { COLOR_CHANGED } else { COLOR_NORMAL },
                    );
                }

                append_text(&mut job, " ", &mono, COLOR_ASCII);
                for addr in base..base + BYTES_PER_ROW {
                    let ch = snapshot
                        .bytes()
                        .get(addr)
                        .copied()
                        .map(ascii_byte)
                        .unwrap_or(' ');
                    append_text(&mut job, &ch.to_string(), &mono, COLOR_ASCII);
                }
                ui.label(job);
            }
        });
    }
}

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

struct CheatSearchUi {
    search: CheatSearch,
    filter_kind: FilterKind,
    filter_value: String,
    new_cheat_addr: String,
    new_cheat_value: String,
}

impl CheatSearchUi {
    fn new() -> Self {
        Self {
            search: CheatSearch::new(0),
            filter_kind: FilterKind::Equal,
            filter_value: String::new(),
            new_cheat_addr: String::new(),
            new_cheat_value: String::new(),
        }
    }

    fn show(
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

fn segment_from_region(region: MemoryRegion, start: usize, len: usize) -> MemorySegment {
    MemorySegment {
        id: region.id.to_string(),
        label: region.label.to_string(),
        start,
        len,
        writable: region.writable,
    }
}

fn format_segment_addr(segment: &MemorySegment, local: usize) -> String {
    if segment.id == "wram" && segment.len == 0x2_0000 {
        let bank = 0x7E + (local >> 16);
        return format!("{bank:02X}:{:04X}", local & 0xFFFF);
    }
    if segment.id == "sram" {
        return format!("S:{local:04X}");
    }
    format!("{}:{local:04X}", segment.id)
}

fn parse_hex(input: &str) -> Option<u32> {
    let value = input
        .trim()
        .trim_start_matches('$')
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    u32::from_str_radix(value, 16).ok()
}

fn append_text(job: &mut LayoutJob, text: &str, font: &FontId, color: Color32) {
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id: font.clone(),
            color,
            ..Default::default()
        },
    );
}

fn ascii_byte(byte: u8) -> char {
    if (0x20..=0x7E).contains(&byte) {
        byte as char
    } else {
        '.'
    }
}
