//! 有道网页表单协议，不注入 Cookie、密钥或大模型会话字段。

use std::sync::LazyLock;
use std::time::{Duration, Instant};

use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, ORIGIN, REFERER};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::{PredictConfig, PredictError};

const ENDPOINT: &str = "https://aidemo.youdao.com/trans";

/// 公开接口无配额承诺；两个功能合并串行，至少隔六秒，遇限频暂停一分钟。
const REQUEST_INTERVAL: Duration = Duration::from_secs(6);
const RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(60);
static NEXT_REQUEST: LazyLock<Mutex<Instant>> = LazyLock::new(|| Mutex::new(Instant::now()));

pub(super) struct YoudaoClient {
    http: reqwest::Client,

    endpoint: String,
}

impl YoudaoClient {
    pub fn new(config: &PredictConfig) -> Result<Self, PredictError> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(ORIGIN, HeaderValue::from_static("https://ai.youdao.com"));
        headers.insert(REFERER, HeaderValue::from_static("https://ai.youdao.com/"));
        Ok(Self {
            http: reqwest::Client::builder()
                .default_headers(headers)
                .user_agent("Qingjian/YoudaoPublic")
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_millis(config.timeout_ms))
                .build()?,
            endpoint: ENDPOINT.into(),
        })
    }

    pub async fn translate(
        &self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<String, PredictError> {
        validate_input(text, target)?;
        let mut next = NEXT_REQUEST.lock().await;
        tokio::time::sleep(next.saturating_duration_since(Instant::now())).await;
        *next = Instant::now() + REQUEST_INTERVAL;
        let result = self.send(text, source, target).await;
        if is_rate_limit(&result) {
            *next = Instant::now() + RATE_LIMIT_COOLDOWN;
        }
        result
    }

    pub(super) async fn send(
        &self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<String, PredictError> {
        let response = self
            .http
            .post(&self.endpoint)
            .form(&[
                ("q", text),
                ("from", source),
                ("to", language_code(target)?),
            ])
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            // error_for_status 不拒绝 3xx，而网页重定向也不能当翻译成功。
            response.error_for_status()?;
            return Err(PredictError::YoudaoResponse);
        }
        parse_response(&response.json::<Value>().await?)
    }

    #[cfg(test)]
    pub(super) fn with_endpoint(config: &PredictConfig, endpoint: String) -> Self {
        let mut client = Self::new(config).unwrap();
        client.endpoint = endpoint;
        client
    }
}

pub(super) fn language_code(code: &str) -> Result<&str, PredictError> {
    match code {
        "zh" | "zh-CHS" => Ok("zh-CHS"),
        "en" | "ja" | "es" => Ok(code),
        _ => Err(PredictError::YoudaoInput),
    }
}

pub(super) fn validate_input(text: &str, target: &str) -> Result<(), PredictError> {
    if text.trim().is_empty() || text.chars().count() > 1000 {
        return Err(PredictError::YoudaoInput);
    }
    language_code(target)?;
    Ok(())
}

pub(super) fn parse_response(payload: &Value) -> Result<String, PredictError> {
    let code = match &payload["errorCode"] {
        Value::String(code) => code.clone(),
        Value::Number(code) => code.to_string(),
        _ => return Err(PredictError::YoudaoResponse),
    };
    if code != "0" {
        return Err(PredictError::YoudaoBusiness(code));
    }
    let lines = payload["translation"]
        .as_array()
        .ok_or(PredictError::YoudaoResponse)?;
    if lines.is_empty() || lines.iter().any(|line| !line.is_string()) {
        return Err(PredictError::YoudaoResponse);
    }
    let text = lines
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    if text.trim().is_empty() {
        return Err(PredictError::EmptyReply);
    }
    Ok(text)
}

fn is_rate_limit(result: &Result<String, PredictError>) -> bool {
    match result {
        Err(PredictError::YoudaoBusiness(code)) => matches!(code.as_str(), "410" | "411" | "413"),
        Err(PredictError::Http(error)) => {
            error.status() == Some(reqwest::StatusCode::TOO_MANY_REQUESTS)
        }
        _ => false,
    }
}
