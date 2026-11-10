use serde::Deserialize;
use sha2::{Digest, Sha256};
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileSearchConfig {
    endpoint: Option<String>,
    model: Option<String>,
    dimensions: Option<usize>,
}

#[derive(Clone)]
pub struct SearchConfig {
    pub endpoint: Option<String>,
    pub model: String,
    pub dimensions: usize,
    pub api_key: Option<String>,
}

impl std::fmt::Debug for SearchConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchConfig")
            .field("model", &self.model)
            .field("dimensions", &self.dimensions)
            .field("configured", &self.endpoint.is_some())
            .finish_non_exhaustive()
    }
}

impl SearchConfig {
    pub fn load(file: FileSearchConfig) -> Self {
        let env = |name| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        Self {
            endpoint: env("EMBEDDING_ENDPOINT")
                .or(file.endpoint)
                .filter(|v| !v.trim().is_empty()),
            model: env("EMBEDDING_MODEL").or(file.model).unwrap_or_default(),
            dimensions: env("EMBEDDING_DIMENSIONS")
                .and_then(|v| v.parse().ok())
                .or(file.dimensions)
                .unwrap_or(0),
            api_key: env("EMBEDDING_API_KEY"),
        }
    }
    pub fn profile(&self) -> String {
        hex::encode(Sha256::digest(format!(
            "v1:{}:{}:{}",
            self.endpoint.as_deref().unwrap_or_default(),
            self.model,
            self.dimensions
        )))
    }
}
