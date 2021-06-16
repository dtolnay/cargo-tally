// Mirrored from https://github.com/rust-lang/crates.io/blob/54a3f10794db7f57e3602426389c369290a8a3d5/src/models/krate.rs

pub const MAX_NAME_LENGTH: usize = 64;

pub fn valid(name: &str) -> bool {
    let under_max_length = name.chars().take(MAX_NAME_LENGTH + 1).count() <= MAX_NAME_LENGTH;
    valid_ident(name) && under_max_length
}

fn valid_ident(name: &str) -> bool {
    valid_feature_prefix(name) && name.chars().next().map_or(false, char::is_alphabetic)
}

fn valid_feature_prefix(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
