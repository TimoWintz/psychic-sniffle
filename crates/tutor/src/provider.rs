//! The provider interface and the adapter for the "chat completions" request format.
//!
//! A provider takes a conversation and streams back text. That is the whole interface, so the rest
//! of the tutor does not know or care which service answers.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read};
use std::time::Duration;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatRequest {
    pub system: String,
    pub messages: Vec<Message>,
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum TutorError {
    #[error("Could not reach the tutor service: {0}")]
    Network(String),
    #[error("The tutor service answered with an error ({status}): {message}")]
    Service { status: u16, message: String },
    #[error("The tutor service sent a reply that could not be read: {0}")]
    Protocol(String),
    #[error("Tutor settings: {0}")]
    Settings(String),
}

pub trait Provider: Send + Sync {
    /// Send the conversation and call `on_text` with each piece of the reply as it arrives.
    /// `on_text` returns `false` to stop early (the listener has gone away).
    fn stream_chat(&self, request: &ChatRequest, on_text: &mut dyn FnMut(&str) -> bool) -> Result<(), TutorError>;

    /// A short description for the settings display: "model-name at host".
    fn describe(&self) -> String;
}

/// Adapter for services that accept `POST {base_url}/chat/completions` with a streamed reply
/// (OpenRouter, OpenAI, most local model servers).
#[derive(Clone, Debug)]
pub struct ChatCompletions {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub max_tokens: Option<u32>,
}

impl ChatCompletions {
    fn url(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }

    fn body(&self, request: &ChatRequest) -> serde_json::Value {
        let mut messages = vec![serde_json::json!({ "role": "system", "content": request.system })];
        messages.extend(request.messages.iter().map(|m| serde_json::json!({ "role": m.role, "content": m.content })));
        let mut body = serde_json::json!({ "model": self.model, "stream": true, "messages": messages });
        if let Some(max) = self.max_tokens {
            body["max_tokens"] = max.into();
        }
        body
    }
}

impl Provider for ChatCompletions {
    fn stream_chat(&self, request: &ChatRequest, on_text: &mut dyn FnMut(&str) -> bool) -> Result<(), TutorError> {
        use ureq::tls::{RootCerts, TlsConfig};
        let agent: ureq::Agent = ureq::Agent::config_builder()
            // Read error bodies ourselves: they carry the service's explanation.
            .http_status_as_error(false)
            // Trust what the operating system trusts, so the game works behind a company proxy.
            .tls_config(TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build())
            .timeout_connect(Some(Duration::from_secs(20)))
            .timeout_recv_response(Some(Duration::from_secs(120)))
            .build()
            .into();

        let mut call = agent.post(self.url()).header("Accept", "text/event-stream");
        if let Some(key) = &self.api_key {
            call = call.header("Authorization", format!("Bearer {key}"));
        }
        let response = call.send_json(self.body(request)).map_err(|e| TutorError::Network(e.to_string()))?;

        let status = response.status().as_u16();
        let mut reader = response.into_body().into_reader();
        if !(200..300).contains(&status) {
            let mut text = String::new();
            let _ = reader.by_ref().take(64 * 1024).read_to_string(&mut text);
            return Err(TutorError::Service { status, message: error_message(&text) });
        }
        read_event_stream(BufReader::new(reader), on_text)
    }

    fn describe(&self) -> String {
        let host = self.base_url.split("//").nth(1).unwrap_or(&self.base_url);
        format!("{} at {}", self.model, host.split('/').next().unwrap_or(host))
    }
}

/// Pull the human-readable message out of an error body, which is usually
/// `{"error": {"message": "..."}}` but varies between services.
fn error_message(body: &str) -> String {
    let fallback = || {
        let trimmed = body.trim();
        if trimmed.is_empty() { "no details given".to_string() } else { trimmed.chars().take(300).collect() }
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(body) else {
        return fallback();
    };
    json.pointer("/error/message")
        .or_else(|| json.pointer("/error"))
        .or_else(|| json.pointer("/message"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(fallback)
}

/// Read a server-sent event stream of chat completion chunks.
fn read_event_stream(reader: impl BufRead, on_text: &mut dyn FnMut(&str) -> bool) -> Result<(), TutorError> {
    let mut received_anything = false;
    for line in reader.lines() {
        let line = line.map_err(|e| TutorError::Network(e.to_string()))?;
        // Blank lines separate events; lines starting with ':' are keep-alive comments.
        let Some(data) = line.strip_prefix("data:") else { continue };
        let data = data.trim();
        if data == "[DONE]" {
            return Ok(());
        }
        let chunk: serde_json::Value =
            serde_json::from_str(data).map_err(|e| TutorError::Protocol(format!("{e} in '{data}'")))?;
        if let Some(error) = chunk.get("error") {
            let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("unspecified error");
            return Err(TutorError::Service { status: 200, message: message.to_string() });
        }
        if let Some(text) = chunk.pointer("/choices/0/delta/content").and_then(|c| c.as_str())
            && !text.is_empty()
        {
            received_anything = true;
            if !on_text(text) {
                return Ok(());
            }
        }
    }
    if received_anything {
        // Some servers close the connection without the final marker.
        Ok(())
    } else {
        Err(TutorError::Protocol("the reply was empty".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(stream: &str) -> Result<String, TutorError> {
        let mut out = String::new();
        read_event_stream(stream.as_bytes(), &mut |t| {
            out.push_str(t);
            true
        })?;
        Ok(out)
    }

    #[test]
    fn reads_streamed_chunks() {
        let stream = "\
: keep-alive comment\n\
data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"content\":\"What have \"}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"content\":\"you measured?\"}}]}\n\n\
data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n\
data: [DONE]\n\n";
        assert_eq!(collect(stream).unwrap(), "What have you measured?");
    }

    #[test]
    fn stream_errors_are_reported() {
        let stream = "data: {\"error\":{\"message\":\"model overloaded\"}}\n\n";
        assert_eq!(
            collect(stream),
            Err(TutorError::Service { status: 200, message: "model overloaded".into() })
        );
        assert!(matches!(collect("data: not json\n\n"), Err(TutorError::Protocol(_))));
        assert!(matches!(collect(""), Err(TutorError::Protocol(_))));
    }

    #[test]
    fn listener_can_stop_early() {
        let stream = "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n\n";
        let mut seen = String::new();
        read_event_stream(stream.as_bytes(), &mut |t| {
            seen.push_str(t);
            false
        })
        .unwrap();
        assert_eq!(seen, "a");
    }

    #[test]
    fn error_bodies() {
        assert_eq!(error_message(r#"{"error":{"message":"bad key","code":401}}"#), "bad key");
        assert_eq!(error_message(r#"{"error":"nope"}"#), "nope");
        assert_eq!(error_message("<html>502</html>"), "<html>502</html>");
        assert_eq!(error_message(""), "no details given");
    }

    #[test]
    fn request_body_shape() {
        let provider = ChatCompletions {
            base_url: "https://example.test/api/v1/".into(),
            model: "some-model".into(),
            api_key: None,
            max_tokens: Some(400),
        };
        assert_eq!(provider.url(), "https://example.test/api/v1/chat/completions");
        assert_eq!(provider.describe(), "some-model at example.test");
        let body = provider.body(&ChatRequest {
            system: "be brief".into(),
            messages: vec![Message { role: Role::User, content: "hi".into() }],
        });
        assert_eq!(
            body,
            serde_json::json!({
                "model": "some-model",
                "stream": true,
                "max_tokens": 400,
                "messages": [
                    { "role": "system", "content": "be brief" },
                    { "role": "user", "content": "hi" },
                ],
            })
        );
    }
}
