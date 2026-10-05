//! 公开接口的编码、响应校验与释义质量边界；默认测试不访问公网。

use super::client::{YoudaoClient, language_code, parse_response, validate_input};
use super::gloss_filler::filled_gloss;
use crate::{PredictConfig, PredictError};
use qingjian_core::Language;
use serde_json::json;

#[test]
fn language_mapping_and_text_limits() {
    for (input, expected) in [("zh", "zh-CHS"), ("en", "en"), ("ja", "ja"), ("es", "es")] {
        assert_eq!(language_code(input).unwrap(), expected);
    }
    assert!(validate_input("", "en").is_err());
    assert!(validate_input(" \n", "en").is_err());
    assert!(validate_input(&"字".repeat(1001), "en").is_err());
    assert!(validate_input(&"字".repeat(1000), "en").is_ok());
    assert!(validate_input("hello", "auto").is_err());
}

#[test]
fn response_requires_business_success_and_usable_strings() {
    for code in [json!(0), json!("0")] {
        assert_eq!(
            parse_response(&json!({"errorCode":code,"translation":["一", "二"]})).unwrap(),
            "一\n二"
        );
    }
    assert!(
        matches!(parse_response(&json!({"errorCode":"411", "translation":["fake"]})), Err(PredictError::YoudaoBusiness(code)) if code == "411")
    );
    for payload in [
        json!(null),
        json!({}),
        json!({"translation":["fake"]}),
        json!({"errorCode":0}),
        json!({"errorCode":0,"translation":[]}),
        json!({"errorCode":0,"translation":[1]}),
    ] {
        assert!(matches!(
            parse_response(&payload),
            Err(PredictError::YoudaoResponse)
        ));
    }
    assert!(matches!(
        parse_response(&json!({"errorCode":0,"translation":[" "]})),
        Err(PredictError::EmptyReply)
    ));
}

#[test]
fn gloss_keeps_short_target_text_without_inventing_metadata() {
    for (language, text) in [
        (Language::English, "develop"),
        (Language::Japanese, "開発する"),
        (Language::Spanish, "desarrollar"),
    ] {
        let filled = filled_gloss("开发", language, text).unwrap();
        assert_eq!(filled.word, "开发");
        assert_eq!(filled.translation.language, language);
        let sense = &filled.translation.senses()[0];
        assert_eq!(sense.text, text);
        assert_eq!(sense.part_of_speech, None);
        assert_eq!(sense.reading, None);
    }
    assert!(filled_gloss("开发", Language::English, "开发").is_none());
    assert!(filled_gloss("开发", Language::Spanish, "develop (something)").is_none());
    assert!(filled_gloss("开发", Language::Japanese, &"あ".repeat(17)).is_none());
    assert!(filled_gloss("开发", Language::Chinese, "develop").is_none());
}

#[test]
fn http_contract_is_form_encoded_without_credentials() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/trans", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(&stream);
        let mut headers = String::new();
        let mut length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some((name, value)) = line.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                length = value.trim().parse::<usize>().unwrap();
            }
            headers.push_str(&line);
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        let reply = r#"{"errorCode":"0","translation":["你好"]}"#;
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", reply.len(), reply).unwrap();
        (headers, String::from_utf8(body).unwrap())
    });
    let client = YoudaoClient::with_endpoint(&PredictConfig::default(), endpoint);
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(client.send("Hello + & 世界", "auto", "zh"))
        .unwrap();
    assert_eq!(result, "你好");
    let (headers, body) = server.join().unwrap();
    let headers = headers.to_ascii_lowercase();
    assert!(headers.starts_with("post /trans "));
    assert!(headers.contains("application/x-www-form-urlencoded"));
    assert!(headers.contains("origin: https://ai.youdao.com"));
    assert!(headers.contains("referer: https://ai.youdao.com/"));
    assert!(!headers.contains("authorization:"));
    assert!(!headers.contains("cookie:"));
    assert_eq!(
        body,
        "q=Hello+%2B+%26+%E4%B8%96%E7%95%8C&from=auto&to=zh-CHS"
    );
}

#[test]
#[ignore = "访问有道公共接口，逐条间隔六秒；仅手动运行"]
fn live_translation_directions() {
    let client = YoudaoClient::new(&PredictConfig {
        timeout_ms: 20000,
        ..Default::default()
    })
    .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for (text, source, target) in [
        ("你好世界", "zh-CHS", "en"),
        ("你好世界", "zh-CHS", "ja"),
        ("你好世界", "zh-CHS", "es"),
        ("Hello world", "auto", "zh"),
        ("こんにちは世界", "auto", "zh"),
        ("Hola mundo", "auto", "zh"),
    ] {
        let translated = runtime
            .block_on(client.translate(text, source, target))
            .unwrap();
        assert!(!translated.trim().is_empty());
        assert_ne!(translated, text);
        println!("{source} → {target}: {translated}");
    }
}
