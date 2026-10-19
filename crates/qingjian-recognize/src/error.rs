//! 识别错误不保留响应正文，避免输入内容进入日志。

#[derive(Debug, thiserror::Error)]
pub enum RecognitionError {
    #[error("network request failed")]
    Network(#[from] reqwest::Error),

    #[error("HTTP status {0}")]
    Http(u16),

    #[error("service error {0}")]
    Service(String),

    #[error("invalid recognition response")]
    Response,

    #[error("invalid media input")]
    Input,
}

impl RecognitionError {
    pub fn user_message(&self) -> String {
        match self {
            Self::Network(error) if error.is_timeout() => "识别超时，请稍后重试".into(),
            Self::Network(_) => "无法连接识别服务，请检查网络".into(),
            Self::Http(429) => "有道请求过于频繁，请稍后重试".into(),
            Self::Http(code) => format!("识别服务 HTTP 错误：{code}"),
            Self::Service(code) if ["410", "411", "413"].contains(&code.as_str()) => {
                "有道接口已限频，请稍后手动重试".into()
            }
            Self::Service(code) if code == "4304" => "未检测到有效语音".into(),
            Self::Service(code) => format!("有道识别失败，错误码：{code}"),
            Self::Response => "识别服务返回了无效响应".into(),
            Self::Input => "图片过大或录音格式无效".into(),
        }
    }
}
