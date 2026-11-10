use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::Semaphore;
use yukina_shared::error::{ApiError, ApiResult};

use yukina_shared::search_config::SearchConfig;

pub struct Embedder {
    pub config: SearchConfig,
    client: reqwest::Client,
    slots: Semaphore,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingItem>,
}
#[derive(Deserialize)]
struct EmbeddingItem {
    index: usize,
    embedding: Vec<f32>,
}
#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

impl Embedder {
    pub fn new(config: SearchConfig) -> Result<Self, String> {
        if let Some(endpoint) = &config.endpoint {
            let url = reqwest::Url::parse(endpoint).map_err(|_| "EMBEDDING_ENDPOINT 格式不正确")?;
            if !matches!(url.scheme(), "http" | "https")
                || config.model.is_empty()
                || !(1..=8192).contains(&config.dimensions)
            {
                return Err("请同时配置 embedding HTTP(S) 地址、模型与 1..8192 维度".into());
            }
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .pool_max_idle_per_host(2)
            .build()
            .map_err(|_| "无法创建 embedding 客户端")?;
        Ok(Self {
            config,
            client,
            slots: Semaphore::new(2),
        })
    }
    pub fn enabled(&self) -> bool {
        self.config.endpoint.is_some()
    }
    pub async fn embed(&self, texts: &[String]) -> ApiResult<Vec<Vec<f32>>> {
        let endpoint = self
            .config
            .endpoint
            .as_ref()
            .ok_or_else(|| ApiError::Unprocessable("语义搜索未配置".into()))?;
        let _permit = tokio::time::timeout(Duration::from_secs(2), self.slots.acquire())
            .await
            .map_err(|_| ApiError::too_many(2))?
            .map_err(|_| ApiError::Internal("索引服务不可用".into()))?;
        let mut request = self.client.post(endpoint).json(&EmbeddingRequest {
            model: &self.config.model,
            input: texts,
        });
        if let Some(key) = &self.config.api_key {
            request = request.bearer_auth(key);
        }
        // 不回传或记录供应商响应正文，避免密钥和文档内容出现在错误日志。
        let mut response = request
            .send()
            .await
            .map_err(|_| ApiError::Internal("embedding 服务连接失败".into()))?;
        if !response.status().is_success() {
            return Err(ApiError::Internal(format!(
                "embedding 服务返回 HTTP {}",
                response.status().as_u16()
            )));
        }
        if response
            .content_length()
            .is_some_and(|n| n > 4 * 1024 * 1024)
        {
            return Err(ApiError::Internal("embedding 响应过大".into()));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| ApiError::Internal("embedding 响应读取失败".into()))?
        {
            if bytes.len() + chunk.len() > 4 * 1024 * 1024 {
                return Err(ApiError::Internal("embedding 响应过大".into()));
            }
            bytes.extend_from_slice(&chunk);
        }
        let mut response: EmbeddingResponse = serde_json::from_slice(&bytes)
            .map_err(|_| ApiError::Internal("embedding 响应格式错误".into()))?;
        response.data.sort_by_key(|item| item.index);
        if response.data.len() != texts.len()
            || response.data.iter().enumerate().any(|(i, item)| {
                item.index != i || !valid_vector(&item.embedding, self.config.dimensions)
            })
        {
            return Err(ApiError::Internal(
                "embedding 数量、维度或数值不正确".into(),
            ));
        }
        Ok(response
            .data
            .into_iter()
            .map(|item| item.embedding)
            .collect())
    }
}

pub fn valid_vector(vector: &[f32], dimensions: usize) -> bool {
    vector.len() == dimensions
        && vector.iter().all(|x| x.is_finite())
        && vector.iter().any(|x| *x != 0.0)
}

pub fn vector_bytes(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|x| x.to_le_bytes()).collect()
}
