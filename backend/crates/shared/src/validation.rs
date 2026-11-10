pub fn valid_url(url: &str) -> bool {
    url.is_empty()
        || ((url.starts_with('/') && !url.starts_with("//")) || url.starts_with("https://"))
            && !url.contains('\\')
            && !url.chars().any(char::is_control)
            && url.len() <= 2000
}
