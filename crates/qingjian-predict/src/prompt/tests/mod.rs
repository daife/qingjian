//! 云预测提示与回复解析测试。
use super::*;

fn request(pinyin: &str, want_sentence: bool) -> PredictionRequest {
    PredictionRequest {
        sequence: 1,
        kind: PredictionKind::Compose,
        before: "我们今天".into(),
        after: String::new(),
        pinyin: pinyin.into(),
        letters: pinyin.replace('\'', ""),
        syllables: 2,
        candidates: vec!["张涛".into(), "张贴".into()],
        guess: String::new(),
        max_items: 2,
        want_sentence,
        text: String::new(),
        target_language: String::new(),
    }
}

#[test]
fn english_completion_preserves_spaces_and_needs_no_pinyin() {
    let mut request = request("", true);
    request.kind = PredictionKind::English;
    request.max_items = 0;
    request.letters = "hello ".into();
    let prompt: serde_json::Value = serde_json::from_str(&user_prompt(&request)).unwrap();
    assert_eq!(prompt["letters"], "hello ");
    assert_eq!(prompt.as_object().unwrap().len(), 2);
    assert_eq!(prompt["before"], "我们今天");
    let reply = parse_reply(
        r#"{"sentence":"hello world","words":[{"text":"wrong","pinyin":""}]}"#,
        &request,
    );
    assert_eq!(reply.sentence.as_deref(), Some("hello world"));
    assert!(reply.words.is_empty());
    assert!(system_prompt(&request).contains("English"));
}

#[test]
fn user_prompt_is_the_request_as_json() {
    let prompt = user_prompt(&request("zhang'tao", true));
    assert!(prompt.contains("\"letters\":\"zhangtao\""));
    assert!(prompt.contains("\"pinyin\":\"zhang'tao\""));
    assert!(prompt.contains("\"syllables\":2"));
    assert!(prompt.contains("\"local_candidates\":[\"张涛\",\"张贴\"]"));
}

#[test]
fn compose_omits_empty_hints_and_selects_task_rules() {
    let mut request = request("zhang'tao", false);
    let user: serde_json::Value = serde_json::from_str(&user_prompt(&request)).unwrap();
    assert!(user.get("after").is_none());
    assert!(user.get("local_sentence").is_none());
    assert!(system_prompt(&request).contains(WORD_RULES));
    assert!(!system_prompt(&request).contains(SENTENCE_RULES));
    request.max_items = 0;
    request.want_sentence = true;
    assert!(!system_prompt(&request).contains(WORD_RULES));
    assert!(system_prompt(&request).contains(SENTENCE_RULES));
    request.candidates.clear();
    let user: serde_json::Value = serde_json::from_str(&user_prompt(&request)).unwrap();
    assert!(user.get("local_candidates").is_none());
}

#[test]
fn reply_keeps_words_with_pinyin_and_drops_the_local_first() {
    let reply = r#"{"words": [{"text": "账套", "pinyin": "zhang tao"}, {"text": "张涛", "pinyin": "zhang tao"},
        {"text": "涨停", "pinyin": ""}, {"text": " 章台 ", "pinyin": "Zhang'Tai"}, {"text": "张套", "pinyin": "zhang tao"}],
        "sentence": " 账套已经建好了\n"}"#;
    let parsed = parse_reply(reply, &request("zhang'tao", true));
    let texts: Vec<(&str, Vec<&str>)> = parsed
        .words
        .iter()
        .map(|w| {
            (
                w.text.as_str(),
                w.syllables.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        texts,
        [
            ("账套", vec!["zhang", "tao"]),
            ("章台", vec!["zhang", "tai"])
        ]
    );
    assert_eq!(parsed.sentence.as_deref(), Some("账套已经建好了"));
    // 没要整句就不收
    assert_eq!(
        parse_reply(reply, &request("zhang'tao", false)).sentence,
        None
    );
    // 整句里抄了 before 的，去掉重叠部分
    let echoed = r#"{"words": [], "sentence": "我们今天账套已经建好了"}"#;
    assert_eq!(
        parse_reply(echoed, &request("zhang'tao", true))
            .sentence
            .as_deref(),
        Some("账套已经建好了")
    );
    let partial = r#"{"words": [], "sentence": "今天账套已经建好了"}"#;
    assert_eq!(
        parse_reply(partial, &request("zhang'tao", true))
            .sentence
            .as_deref(),
        Some("账套已经建好了")
    );
}

#[test]
fn question_replies_keep_answers_with_readings() {
    let mut question = request("san'ge'mu", false);
    question.kind = PredictionKind::Question;
    question.candidates.clear();
    assert!(user_prompt(&question).contains("\"letters\":\"sangemu\""));
    assert!(!user_prompt(&question).contains("before"));
    let reply = r#"{"answers": [{"text": "森", "pinyin": "sēn"}, {"text": "森", "pinyin": "sēn"}, {"text": "巴黎", "pinyin": ""}]}"#;
    let parsed = parse_reply(reply, &question);
    assert_eq!(parsed.words.len(), 2);
    assert_eq!(parsed.words[0].text, "森");
    assert_eq!(parsed.words[0].reading.as_deref(), Some("sēn"));
    assert!(parsed.words[0].syllables.is_empty());
    assert_eq!(parsed.words[1].reading, None);
    assert_eq!(parsed.sentence, None);
}

#[test]
fn garbage_replies_are_empty() {
    assert_eq!(
        parse_reply("not json", &request("k", false)),
        Reply::default()
    );
    assert_eq!(
        parse_reply(r#"{"foo": 1}"#, &request("zt", false)),
        Reply::default()
    );
}

#[test]
fn translate_requests_use_their_own_prompt_and_keep_the_translation() {
    let mut translate = request("", false);
    translate.kind = PredictionKind::Translate;
    translate.text = "我想去吃饭".to_owned();
    translate.target_language = "en".to_owned();
    assert_eq!(system_prompt(&translate), TRANSLATE_SYSTEM_PROMPT);
    assert!(user_prompt(&translate).contains("\"target_language\":\"en\""));
    let reply = parse_reply(r#"{"sentence": "  I want to go eat.\n"}"#, &translate);
    assert_eq!(reply.sentence.as_deref(), Some("I want to go eat."));
    assert!(reply.words.is_empty());
    assert!(
        parse_reply(r#"{"sentence": ""}"#, &translate)
            .sentence
            .is_none()
    );
}

mod scenarios;
mod tasks;

mod review;
