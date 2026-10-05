//! 提示词审阅后的语言、空提示裁剪与翻译方向约束。

use super::{PredictionKind, request};
use crate::prompt::{prediction_prompts, system_prompt};

#[test]
fn absent_local_hints_have_no_explanations_in_either_language() {
    for kind in [PredictionKind::Compose, PredictionKind::English] {
        let mut request = request("prog", true);
        request.kind = kind;
        request.candidates.clear();
        request.guess.clear();
        let prompt = system_prompt(&request);
        assert!(!prompt.contains("local_candidates"));
        assert!(!prompt.contains("local_sentence"));
        request.candidates.push("progress".into());
        assert!(system_prompt(&request).contains("local_candidates"));
        request.max_items = 0;
        assert!(!system_prompt(&request).contains("local_candidates"));
    }
    let mut request = request("ni'hao", true);
    request.guess = "你好".into();
    assert!(system_prompt(&request).contains("local_sentence"));
    request.want_sentence = false;
    assert!(!system_prompt(&request).contains("local_sentence"));
}

#[test]
fn sentence_language_and_unreliable_completion_are_explicit() {
    let mut request = request("ni'hao", true);
    let chinese = system_prompt(&request);
    assert!(chinese.contains("始终用中文"));
    assert!(chinese.contains("没有可靠补全时返回 null"));
    assert!(!chinese.contains("语言随上下文"));
    assert!(!chinese.contains("不增减用户意图的音节"));
    assert!(chinese.contains("纠错后以实际词语读音为准"));
    request.kind = PredictionKind::English;
    let english = system_prompt(&request);
    assert!(english.contains("If no reliable completion exists, return null"));
    assert!(!english.contains("If letters contains spaces"));
    request.want_sentence = false;
    assert!(!system_prompt(&request).contains("return null"));
}

#[test]
fn scenarios_do_not_repeat_priority_or_request_disabled_tasks() {
    let mut request = request("prog", false);
    request.kind = PredictionKind::English;
    let (words, _) = prediction_prompts(&request, "IT");
    assert_eq!(words.matches("Input and context take priority").count(), 1);
    assert!(!words.contains("sentence"));
    request.want_sentence = true;
    request.max_items = 0;
    let (sentence, _) = prediction_prompts(&request, "IT");
    assert!(!sentence.contains("words"));
}

#[test]
fn translation_includes_spanish_and_obeys_target_language() {
    let mut request = request("", false);
    request.kind = PredictionKind::Translate;
    request.target_language = "es".into();
    let prompt = system_prompt(&request);
    assert!(prompt.contains("es 西班牙语"));
    assert!(prompt.contains("不自行切换翻译方向"));
    assert!(prompt.contains("原文已是目标语言时原样返回"));
    assert!(!prompt.contains("非中文改译中文"));
}
