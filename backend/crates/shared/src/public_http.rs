//! 公网 HTTPS 读取：检查全部 DNS 结果、固定连接地址、禁止重定向并限制响应大小。
use crate::error::{ApiError, ApiResult};
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            !(v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_unspecified()
                || v.is_multicast()
                || v.is_broadcast()
                || v.octets()[0] == 0
                || v.octets()[0] >= 240
                || (v.octets()[0] == 100 && (64..=127).contains(&v.octets()[1])))
        }
        IpAddr::V6(v) => v
            .to_ipv4_mapped()
            .map(|v| public_ip(IpAddr::V4(v)))
            .unwrap_or_else(|| {
                !v.is_loopback()
                    && !v.is_unspecified()
                    && !v.is_multicast()
                    && (v.segments()[0] & 0xfe00) != 0xfc00
                    && (v.segments()[0] & 0xffc0) != 0xfe80
            }),
    }
}
pub async fn fetch_bytes(url: reqwest::Url) -> ApiResult<Vec<u8>> {
    let mut checked = url.clone();
    checked.set_query(None);
    if checked.as_str().len() > 2000
        || url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(ApiError::Unprocessable(
            "请求须使用不带账号、密码或片段的HTTPS地址".into(),
        ));
    }
    let host = url.host_str().unwrap();
    let port = url.port_or_known_default().unwrap();
    let addresses = tokio::time::timeout(
        Duration::from_secs(2),
        tokio::net::lookup_host((host, port)),
    )
    .await
    .map_err(|_| ApiError::BadRequest("远程接口解析超时".into()))?
    .map_err(|_| ApiError::BadRequest("远程接口解析失败".into()))?
    .collect::<Vec<SocketAddr>>();
    if addresses.is_empty() || addresses.iter().any(|a| !public_ip(a.ip())) {
        return Err(ApiError::BadRequest("远程接口须使用公网地址".into()));
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(3))
        .resolve_to_addrs(host, &addresses)
        .build()
        .map_err(|_| ApiError::Internal("创建远程请求客户端失败".into()))?;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|_| ApiError::BadRequest("远程接口请求失败".into()))?;
    if !response.status().is_success() || response.content_length().is_some_and(|s| s > 1024 * 1024)
    {
        return Err(ApiError::BadRequest("远程接口未返回有效内容".into()));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ApiError::BadRequest("远程接口读取失败".into()))?
    {
        if bytes.len() + chunk.len() > 1024 * 1024 {
            return Err(ApiError::BadRequest("远程接口响应过大".into()));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_private_and_special_addresses() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.169.254",
            "100.64.0.1",
            "::1",
            "::ffff:127.0.0.1",
            "fc00::1",
            "fe80::1",
        ] {
            assert!(!public_ip(ip.parse().unwrap()), "{ip}");
        }
        assert!(public_ip("1.1.1.1".parse().unwrap()));
    }
}
