//! OpenRouter AI client — two-step analysis pipeline.
//!
//! 1. **Vision model** – receives a screenshot and extracts any visible question.
//! 2. **Reasoning model** – receives the extracted question and solves it.

use base64::Engine as _;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public result types
// ---------------------------------------------------------------------------

/// Output of the vision extraction step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub has_question: bool,
    pub question_text: String,
    pub answer_choices: Vec<String>,
    pub context: String,
    pub is_same_as_previous: bool,
}

pub fn value_to_vec_string(val: &serde_json::Value) -> Vec<String> {
    match val {
        serde_json::Value::String(s) => vec![s.clone()],
        serde_json::Value::Array(arr) => arr
            .iter()
            .map(|v| {
                if let serde_json::Value::String(s) = v {
                    s.clone()
                } else {
                    v.to_string()
                }
            })
            .collect(),
        serde_json::Value::Object(obj) => obj
            .iter()
            .map(|(k, v)| {
                if let serde_json::Value::String(s) = v {
                    format!("{}: {}", k, s)
                } else {
                    format!("{}: {}", k, v)
                }
            })
            .collect(),
        _ => vec![val.to_string()],
    }
}

/// Output of the reasoning solve step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolveResult {
    pub solving_process: String,
    pub recommended_answer: serde_json::Value,
    pub explanation: String,
}

/// Combined result of both steps.
#[derive(Debug, Clone)]
pub struct FullResult {
    pub question_text: String,
    pub answer_choices: Vec<String>,
    pub solving_process: String,
    pub recommended_answer: Vec<String>,
    pub explanation: String,
}

// ---------------------------------------------------------------------------
// API response types (private)
// ---------------------------------------------------------------------------

const API_ENDPOINT: &str = "https://openrouter.ai/api/v1/chat/completions";

#[derive(Deserialize)]
struct ApiResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    content: String,
}

// ---------------------------------------------------------------------------
// Step 1 – Extract question from screenshot
// ---------------------------------------------------------------------------

/// Sends a screenshot to a vision model and extracts any visible question.
///
/// `previous_question` is forwarded so the model can flag duplicates via
/// `is_same_as_previous`.
pub async fn extract_question(
    client: &reqwest::Client,
    api_key: &str,
    vision_model: &str,
    screenshot_png_bytes: &[u8],
    previous_question: Option<&str>,
) -> Result<ExtractionResult, String> {
    let b64 = base64::engine::general_purpose::STANDARD.encode(screenshot_png_bytes);
    let data_url = format!("data:image/png;base64,{b64}");

    let prev = previous_question.unwrap_or("None");

    let prompt = format!(
        r#"Analyze this screenshot. Look for any question, problem, or exam item visible on screen.

Return ONLY valid JSON with this exact structure (no markdown, no code fences):
{{"has_question": true, "question_text": "...", "answer_choices": ["A) ...", "B) ..."], "context": "any relevant context", "is_same_as_previous": false}}

Rules:
- If no question is visible, set has_question to false and leave other fields empty
- Compare with the previous question below to set is_same_as_previous
- Extract ALL answer choices verbatim if multiple choice
- Include any diagrams, code, or context description in the context field

Previous question: {prev}"#
    );

    let body = serde_json::json!({
        "model": vision_model,
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": prompt },
                { "type": "image_url", "image_url": { "url": data_url } }
            ]
        }]
    });

    let content = call_openrouter(client, api_key, &body).await?;
    parse_json_from_content::<ExtractionResult>(&content)
}

// ---------------------------------------------------------------------------
// Step 2 – Solve the extracted question
// ---------------------------------------------------------------------------

/// Sends the extracted question text to a reasoning model and returns the
/// solution.
pub async fn solve_question(
    client: &reqwest::Client,
    api_key: &str,
    reasoning_model: &str,
    question_text: &str,
    answer_choices: &[String],
    context: &str,
) -> Result<SolveResult, String> {
    let choices_str = if answer_choices.is_empty() {
        "None".to_string()
    } else {
        answer_choices.join(", ")
    };

    let prompt = format!(
        r#"Solve this question concisely.

Question: {question_text}
Choices: {choices_str}
Context: {context}

Return ONLY valid JSON (no markdown, no code fences):
{{"solving_process": "Brief 2-3 sentence solution steps", "recommended_answer": "The letter or short answer", "explanation": "One sentence why"}}"#
    );

    let body = serde_json::json!({
        "model": reasoning_model,
        "messages": [{
            "role": "user",
            "content": prompt
        }]
    });

    let content = call_openrouter(client, api_key, &body).await?;
    parse_json_from_content::<SolveResult>(&content)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Performs the actual HTTP POST to OpenRouter and returns the assistant's
/// content string.
async fn call_openrouter(
    client: &reqwest::Client,
    api_key: &str,
    body: &serde_json::Value,
) -> Result<String, String> {
    let resp = client
        .post(API_ENDPOINT)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(body)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {e}"))?;

    if !status.is_success() {
        return Err(format!("OpenRouter returned {status}: {text}"));
    }

    let api_resp: ApiResponse =
        serde_json::from_str(&text).map_err(|e| format!("Failed to parse API response: {e}"))?;

    api_resp
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| "API returned no choices".to_string())
}

/// Parses a JSON value from the model's content string.
///
/// Models sometimes wrap the JSON in markdown fences (```` ```json ... ``` ````)
/// so we try a plain parse first, then fall back to extracting the first
/// `{...}` block.
fn parse_json_from_content<T: serde::de::DeserializeOwned>(content: &str) -> Result<T, String> {
    // Fast path: content is already clean JSON.
    if let Ok(v) = serde_json::from_str::<T>(content.trim()) {
        return Ok(v);
    }

    // Fallback: extract the first JSON object from the content.
    let json_str = extract_json_object(content)
        .ok_or_else(|| format!("No JSON object found in model response: {content}"))?;

    serde_json::from_str::<T>(json_str)
        .map_err(|e| format!("JSON parse error: {e}\nContent: {content}"))
}

/// Returns the first `{…}` substring with balanced braces.
fn extract_json_object(s: &str) -> Option<&str> {
    let start = s.find('{')?;
    let bytes = s.as_bytes();
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if escape_next {
            escape_next = false;
            continue;
        }
        match b {
            b'\\' if in_string => escape_next = true,
            b'"' => in_string = !in_string,
            b'{' if !in_string => depth += 1,
            b'}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_plain() {
        let input = r#"{"has_question": true, "question_text": "hi"}"#;
        assert_eq!(extract_json_object(input), Some(input));
    }

    #[test]
    fn extract_json_with_fences() {
        let input = "```json\n{\"a\": 1}\n```";
        assert_eq!(extract_json_object(input), Some("{\"a\": 1}"));
    }

    #[test]
    fn extract_json_nested() {
        let input = r#"blah {"outer": {"inner": 1}} blah"#;
        assert_eq!(
            extract_json_object(input),
            Some(r#"{"outer": {"inner": 1}}"#)
        );
    }
}
