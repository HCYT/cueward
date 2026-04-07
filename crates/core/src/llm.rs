use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Cue;

#[derive(Debug, Deserialize)]
struct Config {
    llm: LlmConfig,
}

#[derive(Debug, Deserialize)]
struct LlmConfig {
    provider: String,
    api_key: String,
    #[serde(default = "default_model")]
    model: String,
}

fn default_model() -> String {
    "claude-sonnet-4-20250514".into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TriageResult {
    pub summary: String,
    pub tags: Vec<String>,
    pub action_items: Vec<String>,
}

pub struct LlmClient {
    api_key: String,
    model: String,
    provider: String,
}

impl LlmClient {
    fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(home).join(".cueward/config.toml")
    }

    pub fn load() -> Option<Self> {
        let path = Self::config_path();
        let content = fs::read_to_string(&path).ok()?;
        let config: Config = match toml::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("warning: failed to parse {}: {e}", path.display());
                return None;
            }
        };

        // Support env var references like "${ANTHROPIC_API_KEY}"
        let api_key = if config.llm.api_key.starts_with("${") && config.llm.api_key.ends_with('}')
        {
            let var = &config.llm.api_key[2..config.llm.api_key.len() - 1];
            match std::env::var(var) {
                Ok(v) => v,
                Err(_) => {
                    eprintln!("warning: env var {var} not set");
                    return None;
                }
            }
        } else {
            config.llm.api_key
        };

        Some(Self {
            api_key,
            model: config.llm.model,
            provider: config.llm.provider,
        })
    }

    pub fn triage(&self, cue: &Cue) -> Result<TriageResult, String> {
        match self.provider.as_str() {
            "anthropic" => self.call_anthropic(cue),
            other => Err(format!("unsupported provider: {other}")),
        }
    }

    fn call_anthropic(&self, cue: &Cue) -> Result<TriageResult, String> {
        let prompt = build_prompt(cue);

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1024,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        });

        let resp: serde_json::Value = ureq::post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .send_json(&body)
            .map_err(|e| format!("API request failed: {e}"))?
            .body_mut()
            .read_json()
            .map_err(|e| format!("failed to parse API response: {e}"))?;

        let text = resp["content"][0]["text"]
            .as_str()
            .ok_or("no text in API response")?;

        // Extract JSON from response (may be wrapped in markdown code block)
        let json_str = extract_json(text);

        serde_json::from_str::<TriageResult>(json_str)
            .map_err(|e| format!("failed to parse triage result: {e}\nraw: {text}"))
    }
}

fn build_prompt(cue: &Cue) -> String {
    let source = match cue.source {
        crate::CueSource::Safari => "Safari browser history",
        crate::CueSource::Notes => "Apple Notes",
        crate::CueSource::Messages => "iMessage",
    };

    let title = cue.title.as_deref().unwrap_or("");
    let url = cue.url.as_deref().unwrap_or("");

    format!(
        r#"You are a knowledge triage assistant. Analyze this captured knowledge fragment and return a JSON object.

Source: {source}
Title: {title}
URL: {url}
Content:
{content}

Return ONLY a JSON object with these fields:
- "summary": A concise 1-2 sentence summary in the same language as the content
- "tags": An array of relevant topic tags (lowercase, e.g. ["rust", "web-dev", "ai"])
- "action_items": An array of actionable next steps extracted from the content (empty array if none)

JSON:"#,
        content = &cue.content
    )
}

fn extract_json(text: &str) -> &str {
    // Try to find JSON block in markdown code fence
    if let Some(start) = text.find("```json") {
        let start = start + 7;
        if let Some(end) = text[start..].find("```") {
            return text[start..start + end].trim();
        }
    }
    if let Some(start) = text.find("```") {
        let start = start + 3;
        if let Some(end) = text[start..].find("```") {
            return text[start..start + end].trim();
        }
    }
    // Assume raw JSON
    text.trim()
}
