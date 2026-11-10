//! 评论与统计共用的请求规范化规则。

use crate::error::{ApiError, ApiResult};

const MAX_PAGE_LEN: usize = 200;

/// 校验并规范化页面路径：必须以 `/` 开头，去掉查询串与锚点。
pub fn normalize_page(raw: &str) -> ApiResult<String> {
    let page = raw.trim();
    let page = page.split(['?', '#']).next().unwrap_or(page).trim();
    if page.is_empty() || page.chars().count() > MAX_PAGE_LEN || !page.starts_with('/') {
        return Err(ApiError::Unprocessable("页面路径不正确".into()));
    }
    Ok(page.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_identity_ignores_query_and_fragment() {
        for input in [
            " /guestbook/?sort=hot#comments ",
            "/guestbook/#comments?sort=hot",
            "/guestbook/",
        ] {
            assert_eq!(normalize_page(input).unwrap(), "/guestbook/");
        }
    }

    #[test]
    fn invalid_pages_keep_the_validation_status() {
        for input in [
            "",
            "  ",
            "https://example.com/",
            "guestbook",
            "?page=/guestbook/",
        ] {
            assert_eq!(
                normalize_page(input).unwrap_err().status(),
                axum::http::StatusCode::UNPROCESSABLE_ENTITY
            );
        }
    }

    #[test]
    fn path_limit_counts_characters_instead_of_utf8_bytes() {
        assert!(normalize_page(&format!("/{}", "文".repeat(199))).is_ok());
        assert!(normalize_page(&format!("/{}", "文".repeat(200))).is_err());
    }
}
