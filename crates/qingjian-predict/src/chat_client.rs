//! OpenAI 兼容聊天接口，按用途与提供商设置稳定缓存会话。

use std::time::Duration;

use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestUserMessage, CreateChatCompletionRequestArgs,
    CreateChatCompletionResponse, FinishReason, ReasoningEffort, ResponseFormat,
};
use qingjian_core::PredictionRequest;
use reqwest::header::{HeaderMap, HeaderValue};

use crate::cache_session::CacheSession;
use crate::config::PredictConfig;
use crate::error::PredictError;
use crate::prompt::{self, Reply};

/// 联想回复的 token 上限：几条短句足够，防止模型长篇大论。
const MAX_TOKENS: u32 = 200;

/// 采样温度：联想要稳，不要花。
const TEMPERATURE: f32 = 0.3;

/// OpenCode Zen / Go 自 2026-09-06 起要求每个请求带这个头，值是同一会话内稳定的 ID，
/// 他们靠它把同一会话路由到同一上游复用 prompt 缓存，缺了直接 400。
const OPENCODE_SESSION_HEADER: &str = "x-opencode-session";

/// OpenAI 兼容聊天接口的封装：一个请求进、若干联想条目出。
pub struct ChatClient {
    /// 底层客户端。
    clients: [Client<OpenAIConfig>; 5],

    /// 当前提供商支持的缓存会话字段；未知兼容端点不附加参数。
    session_field: Option<&'static str>,

    /// 模型名。
    model: String,

    /// 当前客户端使用的场景；配置变化时客户端与缓存一起重建。
    scenarios: String,

    /// 超时。
    timeout: Duration,

    /// 推理强度；`None` 表示不发这个参数。
    reasoning_effort: Option<ReasoningEffort>,

    /// 接口关思考用的是哪种参数（按接口地址定）。
    thinking_switch: ThinkingSwitch,
}

impl ChatClient {
    pub fn new(config: &PredictConfig, api_key: String) -> Self {
        let openai = OpenAIConfig::new()
            .with_api_base(config.base_url.trim_end_matches('/'))
            .with_api_key(api_key);
        Self {
            clients: CacheSession::ALL.map(|session| {
                Client::with_config(openai.clone())
                    .with_http_client(http_client(&config.base_url, session))
            }),
            session_field: cache_session_field(&config.base_url),
            model: config.model.clone(),
            scenarios: if config.scenarios_enabled {
                config.scenarios.clone()
            } else {
                String::new()
            },
            timeout: Duration::from_millis(config.timeout_ms),
            reasoning_effort: parse_reasoning_effort(&config.reasoning_effort),
            thinking_switch: ThinkingSwitch::for_url(&config.base_url),
        }
    }

    pub async fn complete(&self, request: &PredictionRequest) -> Result<Reply, PredictError> {
        let (system, user) = prompt::prediction_prompts(request, &self.scenarios);
        tracing::debug!(sequence = request.sequence, %user, "联想请求");
        let content = self
            .chat(
                CacheSession::for_prediction(request.kind),
                &system,
                &user,
                MAX_TOKENS,
            )
            .await?;
        Ok(prompt::parse_reply(&content, request))
    }

    /// 一问一答：系统提示 + 用户消息，要 JSON 对象，返回正文。联想与释义兜底共用。
    pub async fn chat(
        &self,
        session: CacheSession,
        system: &str,
        user: &str,
        max_tokens: u32,
    ) -> Result<String, PredictError> {
        let body = self.request_body(session, system, user, max_tokens)?;
        let raw: serde_json::Value = tokio::time::timeout(
            self.timeout,
            self.clients[session as usize].chat().create_byot(body),
        )
        .await
        .map_err(|_| PredictError::Timeout(self.timeout.as_millis() as u64))??;
        let response: CreateChatCompletionResponse = serde_json::from_value(raw.clone())?;
        tracing::debug!(usage = ?response.usage, "模型 token 用量");
        let cut_off = response
            .choices
            .iter()
            .any(|choice| choice.finish_reason == Some(FinishReason::Length));
        let content = response
            .choices
            .into_iter()
            .inspect(
                |choice| tracing::debug!(finish_reason = ?choice.finish_reason, "联想回复结束原因"),
            )
            .find_map(|choice| choice.message.content.filter(|c| !c.trim().is_empty()))
            .ok_or_else(|| {
                // 正文为空时原因五花八门（思考占满额度、模型名不对、接口字段不标准），留下原始响应才查得了
                tracing::warn!(model = %self.model, response = %truncated(&raw), "接口回复里没有正文");
                if cut_off {
                    PredictError::BudgetExhausted
                } else {
                    PredictError::EmptyReply
                }
            })?;
        tracing::debug!(%content, "模型回复");
        Ok(content)
    }

    fn request_body(
        &self,
        session: CacheSession,
        system: &str,
        user: &str,
        max_tokens: u32,
    ) -> Result<serde_json::Value, PredictError> {
        let messages: Vec<ChatCompletionRequestMessage> = vec![
            ChatCompletionRequestSystemMessage::from(system).into(),
            ChatCompletionRequestUserMessage::from(user).into(),
        ];
        let mut args = CreateChatCompletionRequestArgs::default();
        args.model(&self.model)
            .messages(messages)
            .max_tokens(max_tokens)
            .temperature(TEMPERATURE)
            .response_format(ResponseFormat::JsonObject);
        if let Some(effort) = self.reasoning_effort.clone() {
            args.reasoning_effort(effort);
        }
        let mut body = serde_json::to_value(args.build()?)?;
        if let Some(field) = self.session_field {
            body[field] = session.id().into();
        }
        if matches!(self.reasoning_effort, Some(ReasoningEffort::None)) {
            self.thinking_switch.disable(&mut body);
        }
        Ok(body)
    }
}

/// 日志里的原始响应最多留这么多字符。
const LOGGED_RESPONSE_CHARS: usize = 2000;

fn truncated(response: &serde_json::Value) -> String {
    let text = response.to_string();
    match text.char_indices().nth(LOGGED_RESPONSE_CHARS) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text,
    }
}

/// 各家关思考的参数不一样，严格的接口遇到不认识的参数会报 400，所以按接口地址只发对的那个。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThinkingSwitch {
    /// OpenAI 一系：`reasoning_effort: "none"`，请求里已经带了。
    ReasoningEffort,

    /// 智谱（`bigmodel.cn` / `z.ai`）：`thinking: {"type": "disabled"}`，不认 `reasoning_effort`。
    ThinkingType,
}

impl ThinkingSwitch {
    fn for_url(base_url: &str) -> Self {
        let host = host_of(base_url).unwrap_or_default();
        let zhipu = ["bigmodel.cn", "z.ai"]
            .iter()
            .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")));
        if zhipu {
            Self::ThinkingType
        } else {
            Self::ReasoningEffort
        }
    }

    /// 把请求体改成这家接口关思考的写法。
    fn disable(self, body: &mut serde_json::Value) {
        let (Self::ThinkingType, Some(fields)) = (self, body.as_object_mut()) else {
            return;
        };
        fields.remove("reasoning_effort");
        fields.insert(
            "thinking".to_owned(),
            serde_json::json!({ "type": "disabled" }),
        );
    }
}

fn host_of(base_url: &str) -> Option<String> {
    reqwest::Url::parse(base_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
}

/// 只向已知提供商发送其支持的字段，不能把 OpenAI 兼容等同于支持缓存参数。
/// 协议来源见 docs/notes/crate-notes.md。
fn cache_session_field(base_url: &str) -> Option<&'static str> {
    let host = host_of(base_url)?;
    if host == "api.deepseek.com" {
        Some("user_id")
    } else if host == "api.openai.com" {
        Some("prompt_cache_key")
    } else if host == "openrouter.ai" {
        Some("session_id")
    } else {
        None
    }
}

/// 按接口地址决定 HTTP 客户端：OpenCode 带上它要求的会话头，其他服务用默认客户端。
/// 头装不上（理论上不会）就退回默认客户端，请求照发，让服务端的报错说明问题。
fn http_client(base_url: &str, session: CacheSession) -> reqwest::Client {
    if !is_opencode(base_url) {
        return reqwest::Client::new();
    }
    let Ok(value) = HeaderValue::from_str(session.id()) else {
        return reqwest::Client::new();
    };
    let mut headers = HeaderMap::new();
    headers.insert(OPENCODE_SESSION_HEADER, value);
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap_or_default()
}

/// 接口地址是否指向 OpenCode（`opencode.ai` 及其子域）。
fn is_opencode(base_url: &str) -> bool {
    host_of(base_url).is_some_and(|host| host == "opencode.ai" || host.ends_with(".opencode.ai"))
}

/// 配置里的推理强度字符串转成接口枚举；留空不发，认不得的值当留空并记一条警告。
fn parse_reasoning_effort(value: &str) -> Option<ReasoningEffort> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => None,
        "none" => Some(ReasoningEffort::None),
        "minimal" => Some(ReasoningEffort::Minimal),
        "low" => Some(ReasoningEffort::Low),
        "medium" => Some(ReasoningEffort::Medium),
        "high" => Some(ReasoningEffort::High),
        "xhigh" => Some(ReasoningEffort::Xhigh),
        other => {
            tracing::warn!(value = other, "reasoning_effort 不认识，不发这个参数");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasoning_effort_parses_known_values_and_ignores_the_rest() {
        assert!(matches!(
            parse_reasoning_effort("none"),
            Some(ReasoningEffort::None)
        ));
        assert!(matches!(
            parse_reasoning_effort(" High "),
            Some(ReasoningEffort::High)
        ));
        assert!(parse_reasoning_effort("").is_none());
        assert!(parse_reasoning_effort("maximum").is_none());
    }

    #[test]
    fn zhipu_hosts_disable_thinking_with_their_own_field() {
        let mut body = serde_json::json!({ "model": "glm", "reasoning_effort": "none" });
        ThinkingSwitch::for_url("https://open.bigmodel.cn/api/paas/v4").disable(&mut body);
        assert_eq!(body["thinking"]["type"], "disabled");
        assert!(body.get("reasoning_effort").is_none());
        assert_eq!(
            ThinkingSwitch::for_url("https://api.z.ai/api/paas/v4"),
            ThinkingSwitch::ThinkingType
        );

        let mut body = serde_json::json!({ "model": "x", "reasoning_effort": "none" });
        ThinkingSwitch::for_url("https://api.deepseek.com").disable(&mut body);
        assert_eq!(body["reasoning_effort"], "none");
        assert!(body.get("thinking").is_none());
        assert_eq!(
            ThinkingSwitch::for_url("https://example.com/bigmodel.cn"),
            ThinkingSwitch::ReasoningEffort
        );
    }

    #[test]
    fn opencode_is_recognized_by_host_only() {
        assert!(is_opencode("https://opencode.ai/zen/go/v1"));
        assert!(is_opencode("https://OpenCode.ai/zen/v1/"));
        assert!(is_opencode("https://api.opencode.ai/v1"));
        assert!(!is_opencode("https://api.deepseek.com"));
        assert!(!is_opencode("https://example.com/opencode.ai"));
        assert!(!is_opencode("not a url"));
    }

    #[test]
    fn provider_fields_and_all_purpose_ids_reach_the_request_body() {
        for (url, field) in [
            ("https://api.deepseek.com/v1/", "user_id"),
            ("https://api.openai.com/v1", "prompt_cache_key"),
            ("https://openrouter.ai/api/v1", "session_id"),
        ] {
            let config = PredictConfig {
                base_url: url.into(),
                ..Default::default()
            };
            let client = ChatClient::new(&config, "test-key".into());
            let rebuilt = ChatClient::new(&config, "test-key".into());
            for session in CacheSession::ALL {
                let body = client
                    .request_body(session, "system", "first input", 200)
                    .unwrap();
                let next = rebuilt
                    .request_body(session, "system", "other input", 200)
                    .unwrap();
                assert_eq!(body[field], session.id());
                assert_eq!(next[field], body[field]);
                for other in ["user_id", "prompt_cache_key", "session_id"] {
                    if other != field {
                        assert!(body.get(other).is_none_or(serde_json::Value::is_null));
                    }
                }
            }
        }
        for url in [
            "https://api.deepseek.com.example.org/v1",
            "https://example.org/api.openai.com",
            "https://localhost/v1",
            "not a url",
        ] {
            assert_eq!(cache_session_field(url), None);
        }
    }

    /// 真发一个请求到本地端口，看 OpenCode 客户端带了会话头、默认客户端没带。
    #[test]
    fn opencode_client_sends_the_session_header() {
        assert_eq!(
            header_seen_by_server(http_client(
                "https://opencode.ai/zen/go/v1",
                CacheSession::Chinese
            )),
            Some(CacheSession::Chinese.id().to_owned())
        );
        assert_eq!(
            header_seen_by_server(http_client(
                "https://opencode.ai/zen/v1",
                CacheSession::Question
            )),
            Some(CacheSession::Question.id().to_owned())
        );
        assert_eq!(
            header_seen_by_server(http_client(
                "https://api.deepseek.com",
                CacheSession::Chinese
            )),
            None
        );
    }

    /// 起一个只答一次的 HTTP 服务，返回请求里 `x-opencode-session` 的值。
    fn header_seen_by_server(client: reqwest::Client) -> Option<String> {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut header = None;
            for line in BufReader::new(&stream).lines() {
                let line = line.unwrap();
                if line.is_empty() {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case(OPENCODE_SESSION_HEADER)
                {
                    header = Some(value.trim().to_owned());
                }
            }
            stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
                .unwrap();
            header
        });
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async { client.get(&url).send().await.unwrap() });
        server.join().unwrap()
    }
}
