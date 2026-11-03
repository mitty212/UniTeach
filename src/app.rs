use egui::{self, Color32};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

use crate::ai::{self, FullResult};
use crate::capture::{self, MonitorInfo};
use crate::models::ModelCatalog;
use crate::session::{SessionConfig, SessionState, SessionStatus};
use crate::stealth;
use crate::ui;

// Virtual key codes
const VK_CONTROL: i32 = 0x11;
const VK_SHIFT: i32 = 0x10;
const VK_H: i32 = 0x48;
const VK_G: i32 = 0x47;

/// Messages sent from background tasks to the UI.
pub enum BgMessage {
    StatusUpdate(SessionStatus, String),
    ScreenshotTaken(u32),
    NewAnswer(FullResult),
    Error(String),
    QuestionUnchanged,
}

/// Application phases.
#[derive(Debug, Clone, PartialEq)]
pub enum AppPhase {
    Setup,
    Session,
}

pub struct App {
    // Phase
    phase: AppPhase,

    // Setup fields
    monitors: Vec<MonitorInfo>,
    selected_monitor: usize,
    api_key: String,
    selected_interval: usize,
    vision_model_id: String,
    answer_model_id: String,
    model_catalog: ModelCatalog,
    vision_search: String,
    answer_search: String,
    show_api_key: bool,
    setup_error: Option<String>,

    // Session state
    session: Option<SessionState>,
    current_result: Option<FullResult>,
    session_error: Option<String>,
    screenshot_count: u32,
    session_start: Option<Instant>,
    next_capture_at: Option<Instant>,

    // Background communication
    bg_rx: Option<mpsc::Receiver<BgMessage>>,

    // Tokio runtime handle
    runtime: Option<tokio::runtime::Runtime>,

    // Window state
    hwnd: Option<HWND>,
    stealth_applied: bool,
    is_click_through: bool,
    is_visible: bool,

    // Animation counter
    frame_count: u64,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let monitors = capture::list_monitors();

        let mut app = Self {
            phase: AppPhase::Setup,
            monitors,
            selected_monitor: 0,
            api_key: String::new(),
            selected_interval: 0, // 5s default
            vision_model_id: String::new(),
            answer_model_id: String::new(),
            model_catalog: ModelCatalog::default(),
            vision_search: String::new(),
            answer_search: String::new(),
            show_api_key: false,
            setup_error: None,
            session: None,
            current_result: None,
            session_error: None,
            screenshot_count: 0,
            session_start: None,
            next_capture_at: None,
            bg_rx: None,
            runtime: None,
            hwnd: None,
            stealth_applied: false,
            is_click_through: false,
            is_visible: true,
            frame_count: 0,
        };

        app.model_catalog.refresh(&cc.egui_ctx);
        app.spawn_hotkey_thread();
        app
    }

    /// Spawns a background thread to monitor hotkeys even when the UI loop is paused (e.g. when hidden).
    fn spawn_hotkey_thread(&self) {
        std::thread::spawn(|| {
            let mut prev_h = false;
            let mut prev_g = false;
            let mut is_visible = true;
            let mut is_click_through = false;

            loop {
                let ctrl = unsafe { GetAsyncKeyState(VK_CONTROL) } & (0x8000u16 as i16) != 0;
                let shift = unsafe { GetAsyncKeyState(VK_SHIFT) } & (0x8000u16 as i16) != 0;
                let h = unsafe { GetAsyncKeyState(VK_H) } & (0x8000u16 as i16) != 0;
                let g = unsafe { GetAsyncKeyState(VK_G) } & (0x8000u16 as i16) != 0;

                let h_pressed = ctrl && shift && h;
                let g_pressed = ctrl && shift && g;

                // Ctrl+Shift+H: toggle visibility
                if h_pressed && !prev_h {
                    is_visible = !is_visible;
                    if let Some(hwnd) =
                        stealth::find_window_by_title("UniTeach_Overlay_Window_Unique")
                    {
                        stealth::set_window_visible(hwnd, is_visible);
                    }
                }

                // Ctrl+Shift+G: toggle mouse interaction
                if g_pressed && !prev_g {
                    is_click_through = !is_click_through;
                    if let Some(hwnd) =
                        stealth::find_window_by_title("UniTeach_Overlay_Window_Unique")
                    {
                        let _ = stealth::set_click_through(hwnd, is_click_through);
                    }
                }

                prev_h = h_pressed;
                prev_g = g_pressed;
                std::thread::sleep(Duration::from_millis(50));
            }
        });
    }

    /// Try to find and cache our window handle.
    fn ensure_hwnd(&mut self) {
        if self.hwnd.is_none() {
            if let Some(hwnd) = stealth::find_window_by_title("UniTeach_Overlay_Window_Unique") {
                println!("Found overlay HWND: {:?}", hwnd);
                self.hwnd = Some(hwnd);
            }
        }
    }

    /// Apply stealth exclusion to the window.
    fn apply_stealth(&mut self) {
        if self.stealth_applied {
            return;
        }
        if let Some(hwnd) = self.hwnd {
            match stealth::apply_capture_exclusion(hwnd) {
                Ok(()) => {
                    self.stealth_applied = true;
                    println!("Stealth applied successfully.");
                }
                Err(e) => {
                    eprintln!("Failed to apply stealth: {}", e);
                }
            }
        }
    }

    // Hotkeys are now handled by the background thread to work even when hidden.

    /// Process messages from background tasks.
    fn poll_background(&mut self) {
        if let Some(rx) = &self.bg_rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    BgMessage::StatusUpdate(status, text) => {
                        if let Some(session) = &mut self.session {
                            session.status = status;
                            session.status_text = text;
                        }
                    }
                    BgMessage::ScreenshotTaken(count) => {
                        self.screenshot_count = count;
                    }
                    BgMessage::NewAnswer(result) => {
                        self.session_error = None;
                        if let Some(session) = &mut self.session {
                            session.last_question = Some(result.question_text.clone());
                            session.status = SessionStatus::Idle;
                            session.status_text = "Answer ready — resuming monitoring".to_string();
                        }
                        self.current_result = Some(result);
                        // Update next capture time
                        if let Some(session) = &self.session {
                            self.next_capture_at = Some(
                                Instant::now() + Duration::from_secs(session.config.interval_secs),
                            );
                        }
                    }
                    BgMessage::Error(e) => {
                        self.session_error = Some(e);
                        if let Some(session) = &mut self.session {
                            session.status = SessionStatus::Idle;
                            session.status_text = "Error occurred — will retry".to_string();
                        }
                        // Resume capturing after error
                        if let Some(session) = &self.session {
                            self.next_capture_at = Some(
                                Instant::now() + Duration::from_secs(session.config.interval_secs),
                            );
                        }
                    }
                    BgMessage::QuestionUnchanged => {
                        if let Some(session) = &mut self.session {
                            session.status = SessionStatus::Idle;
                            session.status_text = "Same question — waiting".to_string();
                        }
                        if let Some(session) = &self.session {
                            self.next_capture_at = Some(
                                Instant::now() + Duration::from_secs(session.config.interval_secs),
                            );
                        }
                    }
                }
            }
        }
    }

    /// Validate setup inputs.
    fn validate_setup(&self) -> Result<SessionConfig, String> {
        if self.api_key.trim().is_empty() {
            return Err("API key is required".to_string());
        }
        if self.monitors.is_empty() {
            return Err("No monitors detected".to_string());
        }

        if self.selected_monitor >= self.monitors.len() {
            return Err("Select an available monitor".into());
        }
        let vision_model = self
            .model_catalog
            .models
            .iter()
            .find(|m| m.id == self.vision_model_id && m.supports_vision())
            .ok_or("Select an available model that supports image input")?
            .id
            .clone();
        let answer_model = self
            .model_catalog
            .models
            .iter()
            .find(|m| m.id == self.answer_model_id && m.supports_chat())
            .ok_or("Select an available model that supports text chat")?
            .id
            .clone();
        let interval_secs = *ui::setup::INTERVAL_OPTIONS
            .get(self.selected_interval)
            .ok_or("Invalid interval")?;

        Ok(SessionConfig {
            monitor_index: self.selected_monitor,
            api_key: self.api_key.trim().to_string(),
            interval_secs,
            vision_model,
            answer_model,
        })
    }

    /// Start a new session.
    fn start_session(&mut self, config: SessionConfig, ctx: &egui::Context) {
        let session = SessionState::new(config.clone());
        self.session = Some(session);
        self.current_result = None;
        self.session_error = None;
        self.screenshot_count = 0;
        self.session_start = Some(Instant::now());
        self.phase = AppPhase::Session;

        // Keep the session controls usable; the hotkey can enable click-through.
        self.apply_stealth();

        // Start background capture loop
        let (tx, rx) = mpsc::channel();
        self.bg_rx = Some(rx);

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

        let monitor_index = config.monitor_index;
        let api_key = config.api_key.clone();
        let vision_model = config.vision_model.clone();
        let answer_model = config.answer_model.clone();
        let interval = Duration::from_secs(config.interval_secs);
        let ctx_clone = ctx.clone();

        rt.spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default();

            let mut previous_question: Option<String> = None;
            let mut screenshot_count: u32 = 0;

            // Initial delay before first capture
            tokio::time::sleep(Duration::from_secs(2)).await;

            loop {
                // --- Capture ---
                let _ = tx.send(BgMessage::StatusUpdate(
                    SessionStatus::Capturing,
                    "Taking screenshot...".to_string(),
                ));
                ctx_clone.request_repaint();

                let png_bytes = match capture::capture_monitor_png(monitor_index) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        let _ = tx.send(BgMessage::Error(format!("Capture failed: {}", e)));
                        ctx_clone.request_repaint();
                        tokio::time::sleep(interval).await;
                        continue;
                    }
                };

                screenshot_count += 1;
                let _ = tx.send(BgMessage::ScreenshotTaken(screenshot_count));
                ctx_clone.request_repaint();

                // --- Extract question (vision model) ---
                let _ = tx.send(BgMessage::StatusUpdate(
                    SessionStatus::ExtractingQuestion,
                    "Analyzing screenshot...".to_string(),
                ));
                ctx_clone.request_repaint();

                let extraction = match ai::extract_question(
                    &client,
                    &api_key,
                    &vision_model,
                    &png_bytes,
                    previous_question.as_deref(),
                )
                .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = tx.send(BgMessage::Error(format!("Vision API error: {}", e)));
                        ctx_clone.request_repaint();
                        tokio::time::sleep(interval).await;
                        continue;
                    }
                };

                // No question found
                if !extraction.has_question {
                    let _ = tx.send(BgMessage::StatusUpdate(
                        SessionStatus::Idle,
                        "No question detected — waiting".to_string(),
                    ));
                    ctx_clone.request_repaint();
                    tokio::time::sleep(interval).await;
                    continue;
                }

                // Same question as before (already answered)
                if extraction.is_same_as_previous && previous_question.is_some() {
                    let _ = tx.send(BgMessage::QuestionUnchanged);
                    ctx_clone.request_repaint();
                    tokio::time::sleep(interval).await;
                    continue;
                }

                // --- Solve question (reasoning model) ---
                let _ = tx.send(BgMessage::StatusUpdate(
                    SessionStatus::SolvingQuestion,
                    format!(
                        "Solving: {}...",
                        truncate_str(&extraction.question_text, 50)
                    ),
                ));
                ctx_clone.request_repaint();

                let solve_result = match ai::solve_question(
                    &client,
                    &api_key,
                    &answer_model,
                    &extraction.question_text,
                    &extraction.answer_choices,
                    &extraction.context,
                )
                .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = tx.send(BgMessage::Error(format!("Reasoning API error: {}", e)));
                        ctx_clone.request_repaint();
                        tokio::time::sleep(interval).await;
                        continue;
                    }
                };

                // Build full result
                let full_result = FullResult {
                    question_text: extraction.question_text.clone(),
                    answer_choices: extraction.answer_choices,
                    solving_process: solve_result.solving_process,
                    recommended_answer: crate::ai::value_to_vec_string(
                        &solve_result.recommended_answer,
                    ),
                    explanation: solve_result.explanation,
                };

                previous_question = Some(extraction.question_text);

                let _ = tx.send(BgMessage::NewAnswer(full_result));
                ctx_clone.request_repaint();

                // Wait for next interval
                tokio::time::sleep(interval).await;
            }
        });

        self.runtime = Some(rt);
        self.next_capture_at = Some(Instant::now() + Duration::from_secs(2)); // initial delay
    }

    /// Stops the session and returns to setup.
    fn stop_session(&mut self) {
        self.phase = AppPhase::Setup;
        self.session = None;
        self.session_start = None;
        self.current_result = None;
        self.screenshot_count = 0;

        // Un-stealth the window
        if let Some(hwnd) = self.hwnd {
            let _ = stealth::remove_capture_exclusion(hwnd);
            let _ = stealth::set_click_through(hwnd, false);
        }
        self.stealth_applied = false;
        self.is_click_through = false;
        self.runtime = None; // Drop tokio runtime to stop background tasks
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_count = self.frame_count.wrapping_add(1);

        // Find HWND early so hotkeys work
        self.ensure_hwnd();

        // Hotkeys are handled by the background thread.

        // Process background messages
        self.poll_background();
        self.model_catalog
            .poll(&mut self.vision_model_id, &mut self.answer_model_id);

        // Configure egui style
        configure_style(ctx);

        match self.phase {
            AppPhase::Setup => {
                let actions = ui::setup::render_setup(
                    ctx,
                    &self.monitors,
                    &mut self.selected_monitor,
                    &mut self.api_key,
                    &mut self.selected_interval,
                    &mut self.vision_model_id,
                    &mut self.answer_model_id,
                    &self.model_catalog,
                    &mut self.vision_search,
                    &mut self.answer_search,
                    &mut self.show_api_key,
                    &self.setup_error,
                );

                if actions.refresh_models {
                    self.model_catalog.refresh(ctx);
                }
                if actions.start {
                    match self.validate_setup() {
                        Ok(config) => {
                            self.setup_error = None;
                            self.start_session(config, ctx);
                        }
                        Err(e) => {
                            self.setup_error = Some(e);
                        }
                    }
                }
            }
            AppPhase::Session => {
                let elapsed = self
                    .session_start
                    .map(|s| s.elapsed().as_secs())
                    .unwrap_or(0);

                let next_in = self
                    .next_capture_at
                    .map(|t| {
                        if t > Instant::now() {
                            t.duration_since(Instant::now()).as_secs()
                        } else {
                            0
                        }
                    })
                    .unwrap_or(0);

                let status = self
                    .session
                    .as_ref()
                    .map(|s| s.status.clone())
                    .unwrap_or(SessionStatus::Idle);

                let status_text = self
                    .session
                    .as_ref()
                    .map(|s| s.status_text.clone())
                    .unwrap_or_else(|| "Initializing...".to_string());

                let stop_clicked = ui::overlay::render_overlay(
                    ctx,
                    &status,
                    &status_text,
                    self.screenshot_count,
                    elapsed,
                    &self.current_result,
                    next_in,
                    &self.session_error,
                    self.frame_count,
                );

                if stop_clicked {
                    self.stop_session();
                }
            }
        }

        // Request repaint for animations and hotkey polling
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0] // Fully transparent
    }
}

/// Configure the egui visual style for a dark overlay.
fn configure_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.wrap_mode = Some(egui::TextWrapMode::Wrap);
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.interact_size.y = 30.0;
    let vis = &mut style.visuals;

    vis.dark_mode = true;
    vis.override_text_color = Some(Color32::from_rgb(220, 220, 240));
    vis.window_fill = Color32::from_rgba_unmultiplied(20, 20, 35, 220);
    vis.panel_fill = Color32::TRANSPARENT;
    vis.window_fill = Color32::from_rgb(22, 25, 39);

    // Widget colors
    vis.widgets.noninteractive.bg_fill = Color32::from_rgba_unmultiplied(35, 35, 55, 200);
    vis.widgets.inactive.bg_fill = Color32::from_rgba_unmultiplied(40, 40, 65, 200);
    vis.widgets.hovered.bg_fill = Color32::from_rgba_unmultiplied(50, 50, 80, 220);
    vis.widgets.active.bg_fill = Color32::from_rgba_unmultiplied(0, 130, 200, 220);

    // Text input
    vis.extreme_bg_color = Color32::from_rgba_unmultiplied(20, 20, 40, 230);
    vis.selection.bg_fill = Color32::from_rgba_unmultiplied(0, 100, 180, 180);

    // Dropdown/ComboBox background explicitly set to very dark
    vis.widgets.inactive.weak_bg_fill = Color32::from_rgb(15, 15, 20);
    vis.widgets.hovered.weak_bg_fill = Color32::from_rgb(30, 30, 40);
    vis.widgets.active.weak_bg_fill = Color32::from_rgb(50, 50, 60);

    ctx.set_style(style);
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max_len).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::truncate_str;

    #[test]
    fn status_preview_handles_multibyte_questions() {
        assert_eq!(truncate_str("Tiếng Việt 日本語", 8), "Tiếng Vi…");
        assert_eq!(truncate_str("日本語", 3), "日本語");
    }
}
