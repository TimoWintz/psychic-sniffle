//! Tutor settings: which service answers. Nothing is built in; with no settings the tutor is offline.

use crate::provider::{ChatCompletions, Provider, TutorError};
use serde::Deserialize;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TutorSettings {
    /// Address of a service that accepts the chat-completions request format, up to and excluding
    /// `/chat/completions`.
    pub base_url: String,
    /// Model name, spelled as that service expects.
    pub model: String,
    /// Name of the environment variable that holds the key. Omit for a local server without one.
    pub api_key_env: Option<String>,
    /// Upper limit on reply length, if the service should enforce one.
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
struct SettingsFile {
    tutor: Option<TutorSettings>,
}

impl TutorSettings {
    /// Parse a settings file. `Ok(None)` if it has no `[tutor]` section.
    pub fn parse(source: &str) -> Result<Option<TutorSettings>, TutorError> {
        let file: SettingsFile = toml::from_str(source).map_err(|e| TutorError::Settings(e.message().to_string()))?;
        Ok(file.tutor)
    }

    /// Read a settings file. A missing file means "no tutor configured", not an error.
    pub fn load(path: &Path) -> Result<Option<TutorSettings>, TutorError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(TutorError::Settings(format!("cannot read {}: {e}", path.display()))),
        }
    }

    /// Build the provider, reading the key from the environment through `env`.
    pub fn provider_with(&self, env: impl Fn(&str) -> Option<String>) -> Result<Arc<dyn Provider>, TutorError> {
        if !(self.base_url.starts_with("https://") || self.base_url.starts_with("http://")) {
            return Err(TutorError::Settings(format!("base_url '{}' must start with https:// or http://", self.base_url)));
        }
        if self.model.trim().is_empty() {
            return Err(TutorError::Settings("model is empty".into()));
        }
        let api_key = match &self.api_key_env {
            None => None,
            Some(name) => match env(name) {
                Some(key) if !key.trim().is_empty() => Some(key.trim().to_string()),
                _ => {
                    return Err(TutorError::Settings(format!(
                        "the environment variable {name} is not set, so there is no key to send"
                    )));
                }
            },
        };
        Ok(Arc::new(ChatCompletions {
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            api_key,
            max_tokens: self.max_tokens,
        }))
    }

    pub fn provider(&self) -> Result<Arc<dyn Provider>, TutorError> {
        self.provider_with(|name| std::env::var(name).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"
[tutor]
base_url = "https://openrouter.ai/api/v1"
model = "vendor/some-model"
api_key_env = "MY_KEY"
"#;

    #[test]
    fn parses_and_builds() {
        let settings = TutorSettings::parse(EXAMPLE).unwrap().unwrap();
        assert_eq!(settings.api_key_env.as_deref(), Some("MY_KEY"));
        let provider = settings.provider_with(|name| (name == "MY_KEY").then(|| "secret".to_string())).unwrap();
        assert_eq!(provider.describe(), "vendor/some-model at openrouter.ai");
    }

    #[test]
    fn no_section_means_offline() {
        assert_eq!(TutorSettings::parse("").unwrap(), None);
        assert_eq!(TutorSettings::parse("[other]\nx = 1\n").unwrap(), None);
        assert_eq!(TutorSettings::load(Path::new("/nonexistent/settings.toml")).unwrap(), None);
    }

    #[test]
    fn problems_are_explained() {
        let settings = TutorSettings::parse(EXAMPLE).unwrap().unwrap();
        let err = settings.provider_with(|_| None).err().unwrap();
        assert!(err.to_string().contains("MY_KEY is not set"), "{err}");

        let typo = TutorSettings::parse("[tutor]\nbase_url = \"https://x\"\nmodle = \"y\"\n");
        assert!(matches!(typo, Err(TutorError::Settings(_))));

        let local = TutorSettings::parse("[tutor]\nbase_url = \"localhost:8080\"\nmodel = \"m\"\n").unwrap().unwrap();
        assert!(local.provider_with(|_| None).is_err());
        let local = TutorSettings::parse("[tutor]\nbase_url = \"http://localhost:8080/v1\"\nmodel = \"m\"\n").unwrap().unwrap();
        assert!(local.provider_with(|_| None).is_ok());
    }
}
