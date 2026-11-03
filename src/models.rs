//! Live OpenRouter catalogue, in ascending popularity-rank order (rank 1 first).

use std::collections::HashSet;
use std::sync::mpsc;
use std::time::Duration;

use serde::Deserialize;

const MODELS_ENDPOINT: &str = "https://openrouter.ai/api/v1/models";

#[derive(Debug, Clone, Deserialize)]
pub struct Model {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub architecture: Architecture,
    pub endpoints: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Architecture {
    #[serde(default)]
    pub input_modalities: Vec<String>,
    #[serde(default)]
    pub output_modalities: Vec<String>,
}

impl Model {
    pub fn supports_chat(&self) -> bool {
        self.architecture
            .output_modalities
            .iter()
            .any(|m| m == "text")
            && self
                .endpoints
                .as_ref()
                .is_none_or(|endpoints| endpoints.iter().any(|e| e == "/api/v1/chat/completions"))
    }

    pub fn supports_vision(&self) -> bool {
        self.supports_chat()
            && self
                .architecture
                .input_modalities
                .iter()
                .any(|m| m == "image")
    }

    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            &self.id
        } else {
            &self.name
        }
    }

    pub fn matches_search(&self, search: &str) -> bool {
        let search = search.trim().to_lowercase();
        self.id.to_lowercase().contains(&search) || self.name.to_lowercase().contains(&search)
    }
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<Model>,
}

fn parse_models(body: &str) -> Result<Vec<Model>, String> {
    let response: ModelsResponse = serde_json::from_str(body)
        .map_err(|e| format!("Could not read the OpenRouter model catalogue: {e}"))?;
    let mut seen = HashSet::new();
    // Preserve the server's popularity order, including models of every modality.
    let models: Vec<_> = response
        .data
        .into_iter()
        .filter(|m| !m.id.trim().is_empty() && seen.insert(m.id.clone()))
        .collect();
    if models.is_empty() {
        return Err("OpenRouter returned an empty model catalogue. Try refreshing.".into());
    }
    Ok(models)
}

async fn fetch_models() -> Result<Vec<Model>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Could not connect to OpenRouter: {e}"))?;
    let response = client
        .get(MODELS_ENDPOINT)
        .query(&[("output_modalities", "all"), ("sort", "most-popular")])
        .send()
        .await
        .map_err(|e| format!("Could not load OpenRouter models: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "OpenRouter model catalogue returned {status}. Try refreshing."
        ));
    }
    let body = response
        .text()
        .await
        .map_err(|e| format!("Could not read OpenRouter models: {e}"))?;
    parse_models(&body)
}

#[derive(Default)]
pub struct ModelCatalog {
    pub models: Vec<Model>,
    pub loading: bool,
    pub error: Option<String>,
    receiver: Option<mpsc::Receiver<Result<Vec<Model>, String>>>,
}

impl ModelCatalog {
    pub fn refresh(&mut self, ctx: &egui::Context) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.error = None;
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        let ctx = ctx.clone();
        // This public endpoint needs no API key. Keep networking off the UI thread.
        std::thread::spawn(move || {
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Could not start model loading: {e}"))
                .and_then(|runtime| runtime.block_on(fetch_models()));
            let _ = sender.send(result);
            ctx.request_repaint();
        });
    }

    pub fn poll(&mut self, vision_id: &mut String, answer_id: &mut String) {
        let Some(receiver) = &self.receiver else {
            return;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Model loading stopped. Try refreshing.".into())
            }
        };
        self.receiver = None;
        self.loading = false;
        match result {
            Ok(models) => {
                // IDs, rather than indexes, keep selections stable when popularity changes.
                if vision_id.is_empty() {
                    *vision_id = models
                        .iter()
                        .find(|m| m.supports_vision())
                        .map(|m| m.id.clone())
                        .unwrap_or_default();
                }
                if answer_id.is_empty() {
                    *answer_id = models
                        .iter()
                        .find(|m| m.supports_chat())
                        .map(|m| m.id.clone())
                        .unwrap_or_default();
                }
                self.models = models;
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogue() -> Vec<Model> {
        parse_models(r#"{"data":[
            {"id":"z/vision","name":"Vision Z","architecture":{"input_modalities":["text","image"],"output_modalities":["text"]}},
            {"id":"a/text","name":"Text A","architecture":{"input_modalities":["text"],"output_modalities":["text"]}},
            {"id":"a/image","architecture":{"input_modalities":["text"],"output_modalities":["image"]}},
            {"id":"a/embedding","endpoints":["/api/v1/embeddings"],"architecture":{"input_modalities":["text"],"output_modalities":["text"]}},
            {"id":"z/vision"},{"id":""}
        ]}"#).unwrap()
    }

    #[test]
    fn retains_all_modalities_and_server_popularity_order() {
        let models = catalogue();
        assert_eq!(
            models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["z/vision", "a/text", "a/image", "a/embedding"]
        );
        assert!(models[0].supports_vision());
        assert!(models[1].supports_chat());
        assert!(!models[1].supports_vision());
        assert!(!models[2].supports_chat());
        assert!(!models[3].supports_chat());
        assert!(models[0].matches_search(" VISION Z "));
        assert!(models[1].matches_search("A/TEXT"));
    }

    #[test]
    fn refresh_preserves_selected_ids_and_cached_models_on_failure() {
        let mut catalog = ModelCatalog::default();
        let (tx, rx) = mpsc::channel();
        catalog.receiver = Some(rx);
        tx.send(Ok(catalogue())).unwrap();
        let mut vision = "z/vision".to_string();
        let mut answer = "a/text".to_string();
        catalog.poll(&mut vision, &mut answer);
        assert_eq!(answer, "a/text");
        let (tx, rx) = mpsc::channel();
        catalog.receiver = Some(rx);
        tx.send(Err("Offline".into())).unwrap();
        catalog.poll(&mut vision, &mut answer);
        assert_eq!(catalog.models.len(), 4);
        assert_eq!(vision, "z/vision");
        assert_eq!(catalog.error.as_deref(), Some("Offline"));
    }

    #[test]
    fn rejects_empty_or_invalid_catalogues() {
        assert!(parse_models(r#"{"data":[]}"#).is_err());
        assert!(parse_models(r#"{"error":"unavailable"}"#).is_err());
    }

    #[tokio::test]
    #[ignore = "requires network access to the public OpenRouter catalogue"]
    async fn live_catalogue_contains_chat_and_vision_models() {
        let models = fetch_models().await.unwrap();
        assert!(models.len() > 20);
        assert!(models.iter().any(Model::supports_vision));
        assert!(models.iter().any(Model::supports_chat));
        println!("Loaded {} OpenRouter models", models.len());
    }
}
