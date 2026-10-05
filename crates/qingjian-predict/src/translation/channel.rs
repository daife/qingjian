//! 释义渠道，后续翻译服务在装配入口增加实现。
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationChannel {
    #[default]
    Llm,
}

impl TranslationChannel {
    pub const ALL: &[Self] = &[Self::Llm];

    pub fn key(self) -> &'static str {
        match self {
            Self::Llm => "llm",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Llm => "共用大模型云服务",
        }
    }
}
