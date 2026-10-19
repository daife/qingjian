//! Windows 辅助功能配置；渠道独立于输入法的云联想设置。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccessibilityConfig {
    pub ocr_enabled: bool,

    pub ocr_channel: String,

    pub speech_enabled: bool,

    pub speech_channel: String,

    pub speech_language: String,
}

impl Default for AccessibilityConfig {
    fn default() -> Self {
        Self {
            ocr_enabled: false,
            ocr_channel: "youdao_public".into(),
            speech_enabled: false,
            speech_channel: "youdao_public".into(),
            speech_language: "zh-CHS".into(),
        }
    }
}
