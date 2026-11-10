use serde::Deserialize;
#[derive(Debug, Default, Deserialize)]
pub struct FileSummaryConfig {
    endpoint: Option<String>,
    model: Option<String>,
}

#[derive(Clone, Default)]
pub struct SummaryConfig {
    pub endpoint: Option<String>,
    pub model: String,
    pub api_key: Option<String>,
}
impl std::fmt::Debug for SummaryConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SummaryConfig")
            .field("configured", &self.endpoint.is_some())
            .field("model", &self.model)
            .finish()
    }
}
impl SummaryConfig {
    pub fn load(file: FileSummaryConfig) -> Self {
        let env = |name| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        Self {
            endpoint: env("AI_SUMMARY_ENDPOINT")
                .or(file.endpoint)
                .filter(|v| !v.trim().is_empty()),
            model: env("AI_SUMMARY_MODEL").or(file.model).unwrap_or_default(),
            api_key: env("AI_SUMMARY_API_KEY"),
        }
    }
}
