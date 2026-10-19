//! 单次请求、无自动重试；渠道与输入法联想服务互不依赖。

use std::time::Duration;

use base64::Engine;
use reqwest::{Client, Response, multipart};
use serde_json::Value;

use crate::{AUDIO_RATE, MAX_AUDIO_SECONDS, MAX_IMAGE_BYTES, RecognitionError};

pub struct Recognizer {
    client: Client,
}

impl Recognizer {
    pub fn new() -> Result<Self, RecognitionError> {
        Ok(Self {
            client: Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(60))
                .redirect(reqwest::redirect::Policy::none())
                .retry(reqwest::retry::never())
                .user_agent("Qingjian/recognition")
                .build()?,
        })
    }

    pub async fn ocr(&self, png: Vec<u8>) -> Result<String, RecognitionError> {
        if png.len() > MAX_IMAGE_BYTES || !png.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err(RecognitionError::Input);
        }
        let image = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        );
        let response = self
            .client
            .post("https://aidemo.youdao.com/ocrapi1")
            .header("Origin", "https://ai.youdao.com")
            .header("Referer", "https://ai.youdao.com/")
            .form(&[
                ("imgBase", image.as_str()),
                ("lang", "auto"),
                ("company", ""),
            ])
            .send()
            .await?;
        parse_ocr(&payload(response).await?)
    }

    pub async fn speech(&self, wav: Vec<u8>, language: &str) -> Result<String, RecognitionError> {
        if !["zh-CHS", "en"].contains(&language) || !valid_wav(&wav) {
            return Err(RecognitionError::Input);
        }
        let file = multipart::Part::bytes(wav)
            .file_name("blob")
            .mime_str("audio/wav")?;
        let response = self
            .client
            .post("https://aidemo.youdao.com/asr")
            .header("Origin", "https://ai.youdao.com")
            .header("Referer", "https://ai.youdao.com/")
            .query(&[("lang", language), ("mutiSentences", "true")])
            .multipart(multipart::Form::new().part("audioData", file))
            .send()
            .await?;
        parse_speech(&payload(response).await?)
    }
}

async fn payload(mut response: Response) -> Result<Value, RecognitionError> {
    if !response.status().is_success() {
        return Err(RecognitionError::Http(response.status().as_u16()));
    }
    // 禁止异常响应把整个大图片回显进内存。
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err(RecognitionError::Response);
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| RecognitionError::Response)?;
    let code = match &value["errorCode"] {
        Value::String(code) => code.clone(),
        Value::Number(code) => code.to_string(),
        _ => return Err(RecognitionError::Response),
    };
    if code != "0" {
        // 未知服务错误只显示短数字码，不能显示服务返回的输入内容。
        if code.len() > 16 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(RecognitionError::Response);
        }
        return Err(RecognitionError::Service(code));
    }
    Ok(value)
}

fn parse_ocr(value: &Value) -> Result<String, RecognitionError> {
    let Some(lines) = value.get("lines") else {
        return Ok(String::new());
    };
    lines
        .as_array()
        .ok_or(RecognitionError::Response)?
        .iter()
        .map(|line| line["words"].as_str().ok_or(RecognitionError::Response))
        .collect::<Result<Vec<_>, _>>()
        .map(|lines| lines.join("\n"))
}

fn parse_speech(value: &Value) -> Result<String, RecognitionError> {
    value["result"]
        .as_array()
        .ok_or(RecognitionError::Response)?
        .iter()
        .map(|line| line.as_str().ok_or(RecognitionError::Response))
        .collect::<Result<Vec<_>, _>>()
        .map(|lines| lines.join("\n"))
}

fn valid_wav(bytes: &[u8]) -> bool {
    // 采集器输出固定的 44 字节 PCM WAV 头；不接收外部任意 WAV 容器。
    bytes.len() > 44
        && bytes.len() <= 44 + (AUDIO_RATE * MAX_AUDIO_SECONDS * 2) as usize
        && bytes.starts_with(b"RIFF")
        && &bytes[8..16] == b"WAVEfmt "
        && bytes[16..20] == 16u32.to_le_bytes()
        && bytes[20..24] == [1, 0, 1, 0]
        && bytes[24..28] == AUDIO_RATE.to_le_bytes()
        && bytes[32..36] == [2, 0, 16, 0]
        && &bytes[36..40] == b"data"
        && (bytes.len() - 44).is_multiple_of(2)
        && u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize == bytes.len() - 44
        && u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize == bytes.len() - 8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn responses_preserve_lines_and_reject_changed_schema() {
        assert_eq!(
            parse_ocr(&serde_json::json!({"lines":[{"words":"第一行"},{"words":"second"}]}))
                .unwrap(),
            "第一行\nsecond"
        );
        assert_eq!(parse_ocr(&serde_json::json!({})).unwrap(), "");
        assert!(parse_ocr(&serde_json::json!({"lines":[{"text":"wrong"}]})).is_err());
        assert!(parse_speech(&serde_json::json!({"result":"wrong"})).is_err());
        assert_eq!(
            parse_speech(&serde_json::json!({"result":["你好", "世界"]})).unwrap(),
            "你好\n世界"
        );
    }

    #[test]
    fn audio_accepts_sixty_seconds_but_rejects_overflow_and_truncation() {
        fn wav(seconds: u32) -> Vec<u8> {
            let length = AUDIO_RATE * seconds * 2;
            let mut bytes = Vec::new();
            bytes.extend_from_slice(b"RIFF");
            bytes.extend_from_slice(&(length + 36).to_le_bytes());
            bytes.extend_from_slice(b"WAVEfmt ");
            bytes.extend_from_slice(&16u32.to_le_bytes());
            bytes.extend_from_slice(&[1, 0, 1, 0]);
            bytes.extend_from_slice(&AUDIO_RATE.to_le_bytes());
            bytes.extend_from_slice(&(AUDIO_RATE * 2).to_le_bytes());
            bytes.extend_from_slice(&[2, 0, 16, 0]);
            bytes.extend_from_slice(b"data");
            bytes.extend_from_slice(&length.to_le_bytes());
            bytes.resize(44 + length as usize, 0);
            bytes
        }
        let mut full = wav(60);
        assert!(valid_wav(&full));
        assert!(!valid_wav(&wav(61)));
        full.pop();
        assert!(!valid_wav(&full));
        assert!(!valid_wav(&wav(0)));
    }
}
