//! 英文云端候选只补全当前单词，保留已输入前缀。

pub(super) fn is_word_input(input: &str) -> bool {
    !input.is_empty()
        && input
            .chars()
            .all(|c| c.is_ascii_alphabetic() || matches!(c, '\'' | '-'))
}

pub(super) fn matches_prefix(word: &str, input: &str) -> bool {
    is_word_input(input)
        && word.len() > input.len()
        && word
            .to_ascii_lowercase()
            .starts_with(&input.to_ascii_lowercase())
        && word
            .chars()
            .all(|c| c.is_ascii_alphabetic() || matches!(c, '\'' | '-'))
}
