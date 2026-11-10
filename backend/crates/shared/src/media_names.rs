pub fn random_file_name() -> String {
    use rand::Rng;

    hex::encode(rand::thread_rng().gen::<[u8; 16]>())
}

pub fn is_safe_file_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}
