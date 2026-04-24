use std::time::{Duration, Instant};

const HUD_TOAST_DURATION: Duration = Duration::from_millis(1400);
const HUD_TOAST_FONT_SIZE: f32 = 20.0;

#[derive(Debug, Default)]
pub(crate) struct HudToast {
    text: String,
    expires_at: Option<Instant>,
}

impl HudToast {
    pub(crate) fn show(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.expires_at = Some(Instant::now() + HUD_TOAST_DURATION);
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.expires_at
            .is_some_and(|expires_at| Instant::now() < expires_at)
    }

    pub(crate) fn draw(&mut self, ctx: &egui::Context) {
        if !self.is_visible() {
            self.expires_at = None;
            return;
        }

        egui::Area::new(egui::Id::new("state_hud_toast"))
            .anchor(egui::Align2::LEFT_TOP, egui::vec2(12.0, 12.0))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::default()
                    .fill(egui::Color32::from_rgba_premultiplied(18, 18, 18, 220))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(82)))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(&self.text)
                                .strong()
                                .size(HUD_TOAST_FONT_SIZE)
                                .color(egui::Color32::WHITE),
                        );
                    });
            });
    }
}
