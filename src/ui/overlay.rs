use egui::{self, Color32, RichText, Stroke};

use super::{card_frame, resize_handle, title_bar, ACCENT, MUTED};
use crate::ai::FullResult;
use crate::session::SessionStatus;

/// Status and controls stay visible while answers and errors scroll independently.
#[allow(clippy::too_many_arguments)]
pub fn render_overlay(
    ctx: &egui::Context,
    status: &SessionStatus,
    status_text: &str,
    screenshot_count: u32,
    elapsed_secs: u64,
    current_result: &Option<FullResult>,
    next_capture_in: u64,
    error: &Option<String>,
    frame_count: u64,
) -> bool {
    let mut stop_clicked = false;
    title_bar(ctx, "Live session");
    egui::TopBottomPanel::top("session_status")
        .frame(card_frame())
        .show(ctx, |ui| {
            // Reserve room for Stop before allowing the status text to wrap.
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2((ui.available_width() - 80.0).max(1.0), 30.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        if matches!(
                            status,
                            SessionStatus::Capturing
                                | SessionStatus::ExtractingQuestion
                                | SessionStatus::SolvingQuestion
                        ) {
                            ui.spinner();
                        } else {
                            let pulse = (frame_count as f32 * 0.05).sin() * 0.5 + 0.5;
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                            ui.painter().circle_filled(
                                rect.center(),
                                4.0,
                                Color32::from_rgb(45, (170.0 + pulse * 50.0) as u8, 140),
                            );
                        }
                        ui.add(egui::Label::new(RichText::new(status_text).size(13.0)).wrap());
                    },
                );
                stop_clicked = ui
                    .add_sized(
                        [72.0, 30.0],
                        egui::Button::new(
                            RichText::new("Stop").color(Color32::from_rgb(255, 170, 160)),
                        ),
                    )
                    .on_hover_text("Stop session and return to settings")
                    .clicked();
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!(
                        "{}:{:02} elapsed",
                        elapsed_secs / 60,
                        elapsed_secs % 60
                    ))
                    .small()
                    .color(MUTED),
                );
                ui.label(
                    RichText::new(format!("{screenshot_count} captures"))
                        .small()
                        .color(MUTED),
                );
                if matches!(status, SessionStatus::Monitoring | SessionStatus::Idle) {
                    ui.label(
                        RichText::new(format!("Next capture in {next_capture_in}s"))
                            .small()
                            .color(ACCENT),
                    );
                }
            });
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(Color32::TRANSPARENT)
            .inner_margin(egui::Margin::same(8.0)))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("session_content")
                .auto_shrink([false, false]).show(ui, |ui| {
                    if let Some(error) = error {
                        card_frame().stroke(Stroke::new(1.0, Color32::from_rgb(180, 80, 70)))
                            .show(ui, |ui| {
                                ui.colored_label(Color32::from_rgb(255, 165, 145), "The last attempt failed. Retrying automatically.");
                                egui::CollapsingHeader::new("Error details").show(ui, |ui| {
                                    ui.add(egui::Label::new(error).wrap());
                                });
                            });
                        ui.add_space(8.0);
                    }
                    if let Some(result) = current_result {
                        render_answer_panel(ui, result);
                    } else {
                        card_frame().show(ui, |ui| {
                            ui.label(RichText::new("Waiting for a question").size(19.0).color(ACCENT));
                            ui.label("The latest question and answer will appear here after the next capture.");
                            ui.add_space(6.0);
                            ui.label(RichText::new("Ctrl+Shift+H — Show or hide overlay").small().color(MUTED));
                            ui.label(RichText::new("Ctrl+Shift+G — Toggle mouse interaction").small().color(MUTED));
                        });
                    }
                });
        });
    resize_handle(ctx);
    stop_clicked
}

fn render_answer_panel(ui: &mut egui::Ui, result: &FullResult) {
    card_frame().show(ui, |ui| {
        ui.label(
            RichText::new("Question detected")
                .size(16.0)
                .strong()
                .color(ACCENT),
        );
        ui.separator();
        ui.add(egui::Label::new(RichText::new(&result.question_text).size(14.0)).wrap());
        ui.add_space(8.0);

        if !result.recommended_answer.is_empty() {
            ui.label(
                RichText::new("Recommended answer")
                    .strong()
                    .color(Color32::from_rgb(90, 230, 165)),
            );
            ui.add(
                egui::Label::new(
                    RichText::new(result.recommended_answer.join(", "))
                        .size(16.0)
                        .strong()
                        .color(Color32::from_rgb(120, 245, 185)),
                )
                .wrap(),
            );
            ui.add_space(8.0);
        }

        // Display every choice, rather than silently dropping non-recommended choices.
        if !result.answer_choices.is_empty() {
            ui.label(RichText::new("Answer choices").strong().color(MUTED));
            for choice in &result.answer_choices {
                let recommended = result
                    .recommended_answer
                    .iter()
                    .any(|answer| choice_matches(choice, answer));
                let color = if recommended {
                    Color32::from_rgb(110, 240, 175)
                } else {
                    Color32::from_rgb(220, 225, 240)
                };
                let frame = egui::Frame::none()
                    .inner_margin(egui::Margin::same(8.0))
                    .rounding(egui::Rounding::same(6.0))
                    .fill(if recommended {
                        Color32::from_rgb(20, 65, 51)
                    } else {
                        Color32::from_rgb(32, 37, 53)
                    });
                frame.show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.add(egui::Label::new(RichText::new(choice).size(13.0).color(color)).wrap());
                });
            }
            ui.add_space(8.0);
        }
        if !result.explanation.trim().is_empty() {
            ui.label(RichText::new("Explanation").strong().color(MUTED));
            ui.add(egui::Label::new(&result.explanation).wrap());
        }
        if !result.solving_process.trim().is_empty() {
            egui::CollapsingHeader::new("Solution steps").show(ui, |ui| {
                ui.add(egui::Label::new(&result.solving_process).wrap());
            });
        }
    });
}

fn choice_matches(choice: &str, answer: &str) -> bool {
    let choice = choice.trim();
    let answer = answer.trim();
    if answer.is_empty() || choice.is_empty() {
        return false;
    }
    if choice.eq_ignore_ascii_case(answer) {
        return true;
    }
    fn letter(text: &str) -> Option<char> {
        let mut chars = text.chars();
        let first = chars.next()?.to_ascii_uppercase();
        if !first.is_ascii_alphabetic() {
            return None;
        }
        match chars.next() {
            None | Some(')' | '.' | ':' | ' ' | '\t') => Some(first),
            _ => None,
        }
    }
    matches!((letter(choice), letter(answer)), (Some(a), Some(b)) if a == b)
}

#[cfg(test)]
mod tests {
    use super::choice_matches;

    #[test]
    fn highlights_choice_labels_without_matching_word_prefixes() {
        assert!(choice_matches("A) The first choice", "A"));
        assert!(choice_matches(" B. The second choice", "b)"));
        assert!(!choice_matches("Apple", "A"));
        assert!(!choice_matches("A) The first choice", ""));
        assert!(!choice_matches("B) The second choice", "A"));
    }
}
