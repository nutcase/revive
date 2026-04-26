use egui::text::LayoutJob;
use egui::{self, Color32, FontId, RichText};
use revive_cheat::parse_u8_value;

use super::memory::{MemorySnapshot, MemoryWrite};

const BYTES_PER_ROW: usize = 16;
const COLOR_ADDR: Color32 = Color32::from_rgb(0x88, 0x88, 0x88);
const COLOR_NORMAL: Color32 = Color32::from_rgb(0xCC, 0xCC, 0xCC);
const COLOR_CHANGED: Color32 = Color32::from_rgb(0xFF, 0x44, 0x44);
const COLOR_ASCII: Color32 = Color32::from_rgb(0x88, 0xAA, 0x88);

#[derive(Debug)]
pub(crate) struct HexViewerState {
    prev_ram: Vec<u8>,
    goto_addr: String,
    scroll_to_row: Option<usize>,
    edit_addr: String,
    edit_val: String,
}

impl HexViewerState {
    pub(crate) fn new() -> Self {
        Self {
            prev_ram: Vec::new(),
            goto_addr: String::new(),
            scroll_to_row: None,
            edit_addr: String::new(),
            edit_val: String::new(),
        }
    }

    pub(crate) fn update_prev(&mut self, previous: &[u8]) {
        self.prev_ram.clear();
        self.prev_ram.extend_from_slice(previous);
    }

    pub(crate) fn show(
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
