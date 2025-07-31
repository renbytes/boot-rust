// FILE: src/llm_client.rs
use crate::spex_plugin::generate_request::LlmConfig;
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tracing::info;

pub struct LlmClient {
    config: LlmConfig,
    client: Client,
}

impl LlmClient {
    pub fn new(config: LlmConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub async fn generate(&self, prompt: &str) -> Result<String> {
        let timeout = Duration::from_secs(self.config.timeout_s as u64);
        let provider = &self.config.provider;

        let (url, payload) = match provider.as_str() {
            "openai" => (
                format!("{}/chat/completions", self.config.base_url),
                json!({
                    "model": self.config.model,
                    "messages": [{"role": "user", "content": prompt}],
                    "temperature": self.config.temperature,
                }),
            ),
            "gemini" => (
                format!("{}/models/{}:generateContent?key={}", self.config.base_url, self.config.model, self.config.api_key),
                json!({
                    "contents": [{"parts": [{"text": prompt}]}],
                    "generationConfig": {"temperature": self.config.temperature},
                }),
            ),
            _ => return Err(anyhow::anyhow!("Unsupported LLM provider: {}", provider)),
        };

        info!("Sending request to {} model {}", provider, self.config.model);

        let mut builder = self.client.post(&url).timeout(timeout).json(&payload);

        if provider == "openai" {
            builder = builder.bearer_auth(&self.config.api_key);
        }

        let response = builder.send().await?.error_for_status()?;
        let response_json: Value = response.json().await?;

        self.parse_response(provider, &response_json)
    }

    fn parse_response(&self, provider: &str, data: &Value) -> Result<String> {
        let text = match provider {
            "openai" => data["choices"][0]["message"]["content"].as_str(),
            "gemini" => data["candidates"][0]["content"]["parts"][0]["text"].as_str(),
            _ => None,
        };
        text.map(String::from)
            .context(format!("Failed to parse LLM response for {}", provider))
    }
}