pub mod overlay;
pub mod setup;

use egui::{Color32, RichText, Rounding, Stroke};

pub const ACCENT: Color32 = Color32::from_rgb(80, 204, 240);
pub const MUTED: Color32 = Color32::from_rgb(155, 164, 190);

pub fn card_frame() -> egui::Frame {
    egui::Frame::none()
        .fill(Color32::from_rgb(25, 29, 45))
        .rounding(Rounding::same(10.0))
        .stroke(Stroke::new(1.0, Color32::from_rgb(49, 57, 78)))
        .inner_margin(egui::Margin::same(14.0))
}

pub fn title_bar(ctx: &egui::Context, subtitle: &str) {
    egui::TopBottomPanel::top("window_title")
        .frame(
            egui::Frame::none()
                .fill(Color32::from_rgb(19, 22, 35))
                .inner_margin(egui::Margin::symmetric(12.0, 6.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let width = (ui.available_width() - 40.0).max(1.0);
                let response = ui.add_sized(
                    [width, 30.0],
                    egui::Label::new(
                        RichText::new(format!("UniTeach  /  {subtitle}"))
                            .strong()
                            .color(ACCENT),
                    )
                    .truncate()
                    .sense(egui::Sense::drag()),
                );
                if response.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if ui
                    .add_sized([32.0, 30.0], egui::Button::new("×"))
                    .on_hover_text("Close window")
                    .clicked()
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
}

pub fn resize_handle(ctx: &egui::Context) {
    egui::Area::new(egui::Id::new("window_resize"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_BOTTOM, [-1.0, -1.0])
        .show(ctx, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::drag());
            for offset in [3.0, 7.0, 11.0] {
                ui.painter().line_segment(
                    [
                        rect.right_bottom() - egui::vec2(offset, 0.0),
                        rect.right_bottom() - egui::vec2(0.0, offset),
                    ],
                    Stroke::new(1.0, MUTED),
                );
            }
            if response.hovered() || response.dragged() {
                ctx.set_cursor_icon(egui::CursorIcon::ResizeNwSe);
            }
            if response.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(
                    egui::ResizeDirection::SouthEast,
                ));
            }
        });
}
