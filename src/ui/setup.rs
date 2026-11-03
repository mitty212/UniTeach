use egui::{self, Color32, RichText, Vec2};

use super::{card_frame, resize_handle, title_bar, ACCENT, MUTED};
use crate::capture::MonitorInfo;
use crate::models::{Model, ModelCatalog};

pub const INTERVAL_OPTIONS: &[u64] = &[5, 10, 15, 20, 30];

#[derive(Default)]
pub struct SetupActions {
    pub start: bool,
    pub refresh_models: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn render_setup(
    ctx: &egui::Context,
    monitors: &[MonitorInfo],
    selected_monitor: &mut usize,
    api_key: &mut String,
    selected_interval: &mut usize,
    vision_model_id: &mut String,
    answer_model_id: &mut String,
    catalog: &ModelCatalog,
    vision_search: &mut String,
    answer_search: &mut String,
    show_api_key: &mut bool,
    error_msg: &Option<String>,
) -> SetupActions {
    let mut actions = SetupActions::default();
    title_bar(ctx, "Session setup");

    // Reserve the action area before laying out the scrollable settings.
    egui::TopBottomPanel::bottom("setup_actions")
        .frame(
            egui::Frame::none()
                .fill(Color32::from_rgb(19, 22, 35))
                .inner_margin(egui::Margin::symmetric(16.0, 12.0)),
        )
        .show(ctx, |ui| {
            if let Some(error) = error_msg {
                ui.colored_label(Color32::from_rgb(255, 155, 135), error);
                ui.add_space(4.0);
            }
            let ready = !api_key.trim().is_empty()
                && *selected_monitor < monitors.len()
                && catalog
                    .models
                    .iter()
                    .any(|m| m.id == *vision_model_id && m.supports_vision())
                && catalog
                    .models
                    .iter()
                    .any(|m| m.id == *answer_model_id && m.supports_chat());
            let button = egui::Button::new(RichText::new("Start session").size(17.0).strong())
                .fill(Color32::from_rgb(0, 139, 107))
                .min_size(Vec2::new(ui.available_width(), 42.0));
            actions.start = ui
                .add_enabled(ready, button)
                .on_disabled_hover_text(
                    "Enter your API key and select a monitor and compatible models.",
                )
                .clicked();
        });

    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(Color32::from_rgb(15, 18, 29))
                .inner_margin(egui::Margin::same(16.0)),
        )
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("setup_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.label(RichText::new("Set up your overlay").size(25.0).strong());
                    ui.label(
                        RichText::new("Choose a screen, capture interval and OpenRouter models.")
                            .color(MUTED),
                    );
                    ui.add_space(10.0);

                    card_frame().show(ui, |ui| {
                        section_label(ui, "Screen capture");
                        ui.label("Monitor");
                        let monitor_names: Vec<String> = monitors
                            .iter()
                            .enumerate()
                            .map(|(i, m)| {
                                format!("#{} — {} ({} × {})", i + 1, m.name, m.width, m.height)
                            })
                            .collect();
                        let width = ui.available_width();
                        egui::ComboBox::from_id_salt("monitor_select")
                            .selected_text(
                                monitor_names
                                    .get(*selected_monitor)
                                    .map(String::as_str)
                                    .unwrap_or("No monitor detected"),
                            )
                            .width(width)
                            .truncate()
                            .height(180.0)
                            .show_ui(ui, |ui| {
                                ui.set_width((width - 24.0).max(1.0));
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                                for (i, name) in monitor_names.iter().enumerate() {
                                    ui.selectable_value(selected_monitor, i, name);
                                }
                            });
                        if monitors.is_empty() {
                            ui.colored_label(
                                Color32::LIGHT_RED,
                                "No monitor detected. Connect a screen and reopen the app.",
                            );
                        }
                        ui.add_space(8.0);
                        ui.label("Capture interval");
                        ui.horizontal_wrapped(|ui| {
                            for (i, seconds) in INTERVAL_OPTIONS.iter().enumerate() {
                                if ui
                                    .selectable_label(
                                        *selected_interval == i,
                                        format!("{seconds}s"),
                                    )
                                    .clicked()
                                {
                                    *selected_interval = i;
                                }
                            }
                        });
                    });
                    ui.add_space(10.0);

                    card_frame().show(ui, |ui| {
                        section_label(ui, "OpenRouter");
                        ui.label("API key");
                        ui.horizontal(|ui| {
                            let width = (ui.available_width() - 74.0).max(1.0);
                            ui.add_sized(
                                [width, 32.0],
                                egui::TextEdit::singleline(api_key)
                                    .password(!*show_api_key)
                                    .hint_text("sk-or-v1-…"),
                            );
                            if ui
                                .add_sized(
                                    [66.0, 32.0],
                                    egui::Button::new(if *show_api_key { "Hide" } else { "Show" }),
                                )
                                .clicked()
                            {
                                *show_api_key = !*show_api_key;
                            }
                        });
                        ui.label(
                            RichText::new(
                                "Used for this session. The model list loads without a key.",
                            )
                            .small()
                            .color(MUTED),
                        );
                        ui.add_space(8.0);
                        ui.horizontal_wrapped(|ui| {
                            if catalog.loading {
                                ui.spinner();
                                ui.label("Loading models…");
                            } else {
                                ui.label(
                                    RichText::new(format!(
                                        "{} models · Most popular first",
                                        catalog.models.len()
                                    ))
                                    .color(ACCENT),
                                );
                            }
                            actions.refresh_models = ui
                                .add_enabled(!catalog.loading, egui::Button::new("Refresh models"))
                                .clicked();
                        });
                        if let Some(error) = &catalog.error {
                            ui.colored_label(Color32::from_rgb(255, 170, 135), error);
                            if !catalog.models.is_empty() {
                                ui.label(
                                    RichText::new("Showing the last loaded catalogue.")
                                        .small()
                                        .color(MUTED),
                                );
                            }
                        }
                        ui.add_space(8.0);
                        ui.label("Vision model");
                        ui.label(
                            RichText::new(
                                "Reads screenshots. Requires image input and text chat output.",
                            )
                            .small()
                            .color(MUTED),
                        );
                        model_picker(
                            ui,
                            "vision_model",
                            &catalog.models,
                            vision_model_id,
                            vision_search,
                            true,
                        );
                        ui.add_space(8.0);
                        ui.label("Reasoning model");
                        ui.label(
                            RichText::new(
                                "Solves the extracted question. Requires text chat output.",
                            )
                            .small()
                            .color(MUTED),
                        );
                        model_picker(
                            ui,
                            "answer_model",
                            &catalog.models,
                            answer_model_id,
                            answer_search,
                            false,
                        );
                    });
                    ui.add_space(12.0);
                    ui.label(RichText::new("Keyboard shortcuts").strong().color(MUTED));
                    ui.label(
                        RichText::new("Ctrl+Shift+H — Show or hide overlay")
                            .small()
                            .color(MUTED),
                    );
                    ui.label(
                        RichText::new("Ctrl+Shift+G — Toggle mouse interaction")
                            .small()
                            .color(MUTED),
                    );
                    ui.add_space(6.0);
                });
        });
    resize_handle(ctx);
    actions
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(15.0).strong().color(ACCENT));
    ui.add_space(4.0);
}

fn model_picker(
    ui: &mut egui::Ui,
    id: &str,
    models: &[Model],
    selected_id: &mut String,
    search: &mut String,
    vision: bool,
) {
    let selected = models.iter().find(|m| m.id == *selected_id);
    let selected_text = selected.map(Model::display_name).unwrap_or_else(|| {
        if selected_id.is_empty() {
            "Select model"
        } else {
            "Model unavailable — select another"
        }
    });
    let width = ui.available_width();
    let popup_height = (ui.ctx().screen_rect().height() * 0.55).clamp(140.0, 360.0);
    ui.add_enabled_ui(!models.is_empty(), |ui| {
        egui::ComboBox::from_id_salt(id).selected_text(selected_text)
            .width(width).height(popup_height).truncate()
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show_ui(ui, |ui| {
                // ComboBox defaults to extending text; explicitly constrain every popup.
                let content_width = (width - 24.0).max(1.0);
                ui.set_width(content_width);
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                ui.add_sized([content_width, 30.0], egui::TextEdit::singleline(search)
                    .id_salt((id, "search")).hint_text("Search models or providers…"));
                let filtered: Vec<_> = models.iter().filter(|m| m.matches_search(search)).collect();
                ui.label(RichText::new(format!("{} results · Most popular first", filtered.len())).small().color(MUTED));
                ui.separator();
                if filtered.is_empty() {
                    ui.label("No models match your search.");
                }
                egui::ScrollArea::vertical().id_salt((id, "results"))
                    .auto_shrink([false, true]).max_height(popup_height - 88.0)
                    .show_rows(ui, 44.0, filtered.len(), |ui, rows| {
                        for row in rows {
                            let model = filtered[row];
                            let compatible = if vision { model.supports_vision() } else { model.supports_chat() };
                            let text = format!("{}\n{}", model.display_name(), model.id);
                            let response = ui.add_enabled(compatible, egui::Button::new(text)
                                .selected(*selected_id == model.id)
                                .wrap_mode(egui::TextWrapMode::Truncate)
                                .min_size(Vec2::new(ui.available_width(), 44.0)));
                            let response = response.on_hover_text(format!("{}\n{}", model.display_name(), model.id))
                                .on_disabled_hover_text(if vision {
                                    "This model does not support screenshot input with text chat output."
                                } else { "This model does not support text chat output." });
                            if response.clicked() {
                                *selected_id = model.id.clone();
                                ui.memory_mut(|m| m.close_popup());
                            }
                        }
                    });
            }).response.on_hover_text(selected_id.as_str());
    });
}
