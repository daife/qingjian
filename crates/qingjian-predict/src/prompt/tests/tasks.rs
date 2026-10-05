//! 按任务生成最小提示、输入字段与回复结构。

use super::*;

#[test]
fn schema_and_prompts_follow_both_language_task_switches() {
    for kind in [PredictionKind::Compose, PredictionKind::English] {
        for words in [false, true] {
            for sentence in [false, true] {
                let mut request = request("prog", sentence);
                request.kind = kind;
                request.max_items = if words { 2 } else { 0 };
                request.guess = "本地猜测".into();
                let schema = response_schema(&request);
                assert_eq!(schema["properties"].get("words").is_some(), words);
                assert_eq!(schema["properties"].get("sentence").is_some(), sentence);
                assert_eq!(
                    schema["required"].as_array().unwrap().len(),
                    usize::from(words) + usize::from(sentence)
                );
                let prompt = system_prompt(&request);
                if kind == PredictionKind::Compose {
                    assert_eq!(prompt.contains(WORD_RULES), words);
                    assert_eq!(prompt.contains(SENTENCE_RULES), sentence);
                    assert_eq!(prompt.contains("local_sentence"), sentence);
                } else {
                    assert!(!prompt.contains("pinyin"));
                    assert_eq!(prompt.contains("natural complete English"), sentence);
                    assert!(
                        schema["properties"]["words"]["items"]["properties"]
                            .get("pinyin")
                            .is_none()
                    );
                }
                let user: serde_json::Value = serde_json::from_str(&user_prompt(&request)).unwrap();
                assert_eq!(user.get("max_items").is_some(), words);
                assert_eq!(user.get("local_candidates").is_some(), words);
                assert!(user.get("want_sentence").is_none());
                if kind == PredictionKind::Compose {
                    assert_eq!(user.get("local_sentence").is_some(), sentence);
                }
            }
        }
    }
}

#[test]
fn disabled_tasks_discard_unsolicited_fields() {
    let content = r#"{"words":[{"text":"程序","pinyin":"cheng xu"}],"sentence":"程序很好用"}"#;
    let mut request = request("cheng'xu", false);
    assert!(parse_reply(content, &request).sentence.is_none());
    request.max_items = 0;
    assert!(parse_reply(content, &request).is_empty());
    request.want_sentence = true;
    let reply = parse_reply(content, &request);
    assert!(reply.words.is_empty());
    assert!(reply.sentence.is_some());
}

#[test]
fn english_words_need_no_pinyin_and_exclude_local_duplicates() {
    let mut request = request("prog", false);
    request.kind = PredictionKind::English;
    request.candidates = vec!["program".into()];
    let reply = parse_reply(
        r#"{"words":[{"text":"PROGRAM"},{"text":"programming"},{"text":"programming"},{"text":"progress"},{"text":"progressive"}],"sentence":"unrequested"}"#,
        &request,
    );
    assert_eq!(
        reply
            .words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>(),
        ["programming", "progress"]
    );
    assert!(reply.words.iter().all(|word| word.syllables.is_empty()));
    assert!(reply.sentence.is_none());
    request.max_items = 0;
    assert!(parse_reply(r#"{"words":[{"text":"programming"}]}"#, &request).is_empty());
}
