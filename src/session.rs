//! Session configuration and runtime state.
//!
//! These types track what the application is doing across capture–extract–solve
//! cycles.

use std::fmt;
use std::time::Instant;

/// Immutable configuration for a monitoring session.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Index of the monitor to capture (as returned by [`crate::capture::list_monitors`]).
    pub monitor_index: usize,
    /// OpenRouter API key.
    pub api_key: String,
    /// Seconds between capture cycles.
    pub interval_secs: u64,
    /// Model identifier for the vision (extraction) step.
    pub vision_model: String,
    /// Model identifier for the reasoning (solve) step.
    pub answer_model: String,
}

/// Mutable runtime state for the current session.
pub struct SessionState {
    pub config: SessionConfig,
    /// The most recently extracted question text, used for deduplication.
    pub last_question: Option<String>,
    /// The most recent full result (extraction + solve).
    pub last_result: Option<crate::ai::FullResult>,
    /// Total screenshots taken this session.
    pub screenshot_count: u32,
    /// When this session was started.
    pub start_time: Instant,
    /// What the session is currently doing.
    pub status: SessionStatus,
    /// Human-readable status string for display in the overlay.
    pub status_text: String,
    /// Last error message, if any.
    pub error: Option<String>,
}

/// Describes what phase the session loop is in.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionStatus {
    /// Waiting for the next capture interval.
    Monitoring,
    /// Taking a screenshot.
    Capturing,
    /// Sending the screenshot to the vision model.
    ExtractingQuestion,
    /// Sending the question to the reasoning model.
    SolvingQuestion,
    /// Paused / waiting for the next interval tick.
    Idle,
}

impl SessionState {
    /// Creates a fresh state for the given configuration.
    pub fn new(config: SessionConfig) -> Self {
        Self {
            config,
            last_question: None,
            last_result: None,
            screenshot_count: 0,
            start_time: Instant::now(),
            status: SessionStatus::Idle,
            status_text: "Initializing...".to_string(),
            error: None,
        }
    }

    /// Returns a human-readable status string suitable for display in the
    /// overlay UI.
    pub fn status_text(&self) -> String {
        let elapsed = self.start_time.elapsed();
        let mins = elapsed.as_secs() / 60;
        let secs = elapsed.as_secs() % 60;

        let base = match &self.status {
            SessionStatus::Monitoring => "Monitoring screen…".to_string(),
            SessionStatus::Capturing => "Capturing screenshot…".to_string(),
            SessionStatus::ExtractingQuestion => "Extracting question (vision)…".to_string(),
            SessionStatus::SolvingQuestion => "Solving question (reasoning)…".to_string(),
            SessionStatus::Idle => format!("Idle — next capture in {}s", self.config.interval_secs),
        };

        let error_suffix = match &self.error {
            Some(e) => format!(" | ⚠ {e}"),
            None => String::new(),
        };

        format!(
            "{base} | captures: {} | uptime: {mins}m {secs}s{error_suffix}",
            self.screenshot_count
        )
    }
}

impl fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionStatus::Monitoring => write!(f, "Monitoring"),
            SessionStatus::Capturing => write!(f, "Capturing"),
            SessionStatus::ExtractingQuestion => write!(f, "Extracting Question"),
            SessionStatus::SolvingQuestion => write!(f, "Solving Question"),
            SessionStatus::Idle => write!(f, "Idle"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SessionConfig {
        SessionConfig {
            monitor_index: 0,
            api_key: "test-key".to_string(),
            interval_secs: 5,
            vision_model: "google/gemini-2.5-flash".to_string(),
            answer_model: "google/gemini-2.5-flash".to_string(),
        }
    }

    #[test]
    fn new_session_starts_idle() {
        let state = SessionState::new(test_config());
        assert_eq!(state.status, SessionStatus::Idle);
        assert_eq!(state.screenshot_count, 0);
        assert!(state.last_question.is_none());
        assert!(state.last_result.is_none());
        assert!(state.error.is_none());
    }

    #[test]
    fn status_display() {
        assert_eq!(SessionStatus::Monitoring.to_string(), "Monitoring");
        assert_eq!(SessionStatus::Idle.to_string(), "Idle");
    }

    #[test]
    fn status_text_contains_uptime() {
        let state = SessionState::new(test_config());
        let text = state.status_text();
        assert!(text.contains("captures: 0"));
        assert!(text.contains("uptime:"));
    }
}
