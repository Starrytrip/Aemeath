//! 统一 API 错误类型（响应格式 `{"detail": "..."}`）

use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// 所有 handler 返回 `ApiResult<T>`；`T: Serialize` 时自动转 JSON。
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// 400
    #[error("{0}")]
    BadRequest(String),
    /// 401
    #[error("{0}")]
    Unauthorized(String),
    /// 403
    #[error("{0}")]
    Forbidden(String),
    /// 404
    #[error("{0}")]
    NotFound(String),
    /// 409
    #[error("{0}")]
    Conflict(String),
    /// 422
    #[error("{0}")]
    Unprocessable(String),
    /// 429（可带 Retry-After 头）
    #[error("{message}")]
    TooManyRequests {
        message: String,
        retry_after: Option<u64>,
    },
    /// 500
    #[error("内部错误: {0}")]
    Internal(String),
    #[error("数据库操作失败")]
    Database(#[from] sea_orm::DbErr),
    #[error("文件操作失败")]
    Io(#[from] std::io::Error),
    #[error("密码处理失败")]
    Password(#[from] bcrypt::BcryptError),
}

/// handler 的标准返回类型别名。
pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    /// 429：请求过于频繁，附带建议重试等待秒数。
    pub fn too_many(retry_after: u64) -> Self {
        ApiError::TooManyRequests {
            message: "请求过于频繁，请稍后再试".into(),
            retry_after: Some(retry_after),
        }
    }

    /// 错误对应的 HTTP 状态码。
    pub fn status(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden(_) => StatusCode::FORBIDDEN,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::Unprocessable(_) => StatusCode::UNPROCESSABLE_ENTITY,
            ApiError::TooManyRequests { .. } => StatusCode::TOO_MANY_REQUESTS,
            ApiError::Internal(_)
            | ApiError::Database(_)
            | ApiError::Io(_)
            | ApiError::Password(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        let detail = if status.is_server_error() {
            tracing::error!(error = ?self, "API 请求失败");
            "服务暂时不可用，请稍后重试".to_owned()
        } else {
            self.to_string()
        };
        let kind = match &self {
            Self::BadRequest(_) => "bad_request",
            Self::Unauthorized(_) => "unauthorized",
            Self::Forbidden(_) => "forbidden",
            Self::NotFound(_) => "not_found",
            Self::Conflict(_) => "conflict",
            Self::Unprocessable(_) => "unprocessable",
            Self::TooManyRequests { .. } => "rate_limited",
            Self::Internal(_) | Self::Database(_) | Self::Io(_) | Self::Password(_) => "internal",
        };
        let mut response =
            (status, Json(json!({ "detail": detail, "kind": kind }))).into_response();
        if let ApiError::TooManyRequests {
            retry_after: Some(secs),
            ..
        } = &self
        {
            if let Ok(v) = secs.to_string().parse() {
                response.headers_mut().insert(header::RETRY_AFTER, v);
            }
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[tokio::test]
    async fn internal_causes_are_preserved_but_not_sent_to_clients() {
        let error = ApiError::from(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "private/database/path",
        ));
        assert!(error.source().is_some());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["kind"], "internal");
        assert!(!String::from_utf8_lossy(&bytes).contains("private/database/path"));
        let response = ApiError::too_many(7).into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[header::RETRY_AFTER], "7");
    }
}
