//! 固定空格 / 回车长按时执行的动作。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitAction {
    #[default]
    None,
    Candidate,
    Raw,
    FirstTranslation,
    InsertSpace,
}

impl CommitAction {
    pub const ALL: [Self; 5] = [
        Self::None,
        Self::Candidate,
        Self::Raw,
        Self::FirstTranslation,
        Self::InsertSpace,
    ];

    pub const fn enabled(self) -> bool {
        !matches!(self, Self::None)
    }

    pub const fn key(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Candidate => "candidate",
            Self::Raw => "raw",
            Self::FirstTranslation => "first_translation",
            Self::InsertSpace => "insert_space",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "关闭",
            Self::Candidate => "高亮候选上屏",
            Self::Raw => "原样上屏",
            Self::FirstTranslation => "高亮候选的首个译词上屏",
            Self::InsertSpace => "在输入内容中添加一个空格",
        }
    }
}
