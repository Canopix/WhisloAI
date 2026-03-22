use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub(crate) struct ChatRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) model: Option<&'a str>,
    pub(crate) messages: Vec<ChatMessage<'a>>,
    pub(crate) temperature: f32,
}

#[derive(Debug, Serialize)]
pub(crate) struct ChatMessage<'a> {
    pub(crate) role: &'a str,
    pub(crate) content: &'a str,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChatResponse {
    pub(crate) choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChatChoice {
    pub(crate) message: ChatOutput,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChatOutput {
    pub(crate) content: serde_json::Value,
}
