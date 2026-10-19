//! 生僻字来源表中的字符、读音与常用字标记。

/// 生僻字表的一行。
#[derive(Debug, Clone)]
pub struct RareRow {
    pub ch: char,

    /// 无声调读音，没核对过是不是合法音节。
    pub syllables: Vec<String>,

    /// 来源带 `everyday`：日常常用，按规范字处理（读音取 Unihan、词频取语料）。
    pub everyday: bool,
}
