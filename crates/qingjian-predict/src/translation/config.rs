//! 翻译任务共用的配置形状，每项功能各自保存开关与渠道。
use super::TranslationChannel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TranslationConfig {
    pub enabled: bool,

    pub channel: TranslationChannel,
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            channel: TranslationChannel::Llm,
        }
    }
}
