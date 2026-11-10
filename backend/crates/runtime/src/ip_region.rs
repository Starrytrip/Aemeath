//! IP 归属地查询、代理头解析和脱敏回退。

use crate::state::AppState;
use axum::http::HeaderMap;
use std::net::SocketAddr;

/// 脱敏 IP：IPv4 保留前两段，IPv6 保留前两组。
fn mask_ip(ip: &str) -> String {
    let ip = ip.trim();
    if ip.contains('.') {
        let parts: Vec<&str> = ip.split('.').collect();
        if parts.len() == 4 {
            return format!("{}.{}.*.*", parts[0], parts[1]);
        }
        return ip.to_string();
    }
    let groups: Vec<&str> = ip.split(':').collect();
    if groups.len() >= 2 {
        return format!("{}:{}:*", groups[0], groups[1]);
    }
    ip.to_string()
}

/// 把 IP 解析为展示用的来源地：境内「省 市」、境外国家、内网「内网IP」；
/// 无归属地数据库或解析失败时回退为脱敏 IP。入参为 None 时返回 None。
pub fn resolve_region(state: &AppState, ip: Option<&str>) -> Option<String> {
    let ip = ip?;
    if let Some(searcher) = &state.ip_region {
        if let Ok(region) = searcher.search(ip) {
            let parts: Vec<&str> = region.split('|').collect();
            if parts.iter().any(|part| part.contains("内网")) {
                return Some("内网IP".to_string());
            }
            let pick = |value: &str| (!value.is_empty() && value != "0").then(|| value.to_string());
            if let Some(country) = parts.first().and_then(|value| pick(value)) {
                if country != "中国" {
                    return Some(country);
                }
                let province = parts.get(2).and_then(|value| pick(value));
                let city = parts.get(3).and_then(|value| pick(value));
                let mut label: Vec<String> = Vec::new();
                if let Some(value) = province {
                    label.push(value);
                }
                if let Some(value) = city {
                    if label.last().map(String::as_str) != Some(value.as_str()) {
                        label.push(value);
                    }
                }
                return Some(if label.is_empty() {
                    country
                } else {
                    label.join(" ")
                });
            }
        }
    }
    Some(mask_ip(ip))
}

/// 取客户端 IP：优先代理头，其次 TCP 连接地址。
pub fn client_ip(headers: &HeaderMap, addr: SocketAddr) -> String {
    for header in ["x-forwarded-for", "x-real-ip"] {
        if let Some(value) = headers.get(header).and_then(|value| value.to_str().ok()) {
            if let Some(ip) = value
                .split(',')
                .next()
                .map(str::trim)
                .filter(|item| !item.is_empty())
            {
                return ip.to_string();
            }
        }
    }
    addr.ip().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_ip_priority_and_socket_fallback_are_preserved() {
        let addr = "127.0.0.1:8080".parse().unwrap();
        let mut headers = HeaderMap::new();
        assert_eq!(client_ip(&headers, addr), "127.0.0.1");
        headers.insert("x-real-ip", "192.0.2.1".parse().unwrap());
        assert_eq!(client_ip(&headers, addr), "192.0.2.1");
        headers.insert(
            "x-forwarded-for",
            "198.51.100.1, 192.0.2.2".parse().unwrap(),
        );
        assert_eq!(client_ip(&headers, addr), "198.51.100.1");
    }

    #[test]
    fn fallback_masks_ipv4_and_ipv6() {
        assert_eq!(mask_ip("192.0.2.123"), "192.0.*.*");
        assert_eq!(mask_ip("2001:db8:1234:5678::1"), "2001:db8:*");
    }
}
