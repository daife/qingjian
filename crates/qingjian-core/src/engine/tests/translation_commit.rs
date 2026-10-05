//! 译词上屏托底：候选消耗、整句、隐私、过期响应与缺少第二条释义的边界。

use crate::{
    Candidate, CandidateKind, Engine, Language, Prediction, PredictionRequest, SelectionTranslator,
    Sense, Translation,
};
use std::sync::{Arc, Mutex};

struct Service {
    sent: Arc<Mutex<Vec<PredictionRequest>>>,

    replies: Arc<Mutex<Vec<Prediction>>>,
}

impl SelectionTranslator for Service {
    fn submit(&mut self, request: PredictionRequest) {
        self.sent.lock().unwrap().push(request);
    }

    fn poll(&mut self) -> Option<Prediction> {
        self.replies.lock().unwrap().pop()
    }
}

type Fixture = (
    Engine,
    Arc<Mutex<Vec<PredictionRequest>>>,
    Arc<Mutex<Vec<Prediction>>>,
);

fn setup() -> Fixture {
    let sent = Arc::default();
    let replies = Arc::default();
    let mut engine = super::engine().with_selection_translator(Box::new(Service {
        sent: Arc::clone(&sent),
        replies: Arc::clone(&replies),
    }));
    engine.set_translation_commit_fallback(true);
    engine.set_input("kaifazhe");
    (engine, sent, replies)
}

fn candidate(engine: &Engine, text: &str) -> Candidate {
    engine
        .query()
        .unwrap()
        .candidates
        .items
        .into_iter()
        .find(|candidate| candidate.text == text)
        .unwrap()
}

fn reply(
    sent: &Arc<Mutex<Vec<PredictionRequest>>>,
    replies: &Arc<Mutex<Vec<Prediction>>>,
    text: Option<&str>,
) {
    let sequence = sent.lock().unwrap().last().unwrap().sequence;
    replies.lock().unwrap().push(Prediction {
        sequence,
        words: Vec::new(),
        sentence: text.map(str::to_owned),
    });
}

#[test]
fn translation_commit_consumes_only_the_selected_word_and_predict_poll_does_not_steal_reply() {
    let (mut engine, sent, replies) = setup();
    let word = candidate(&engine, "开发");
    assert_eq!(engine.commit_translation(&word, 0), None);
    assert_eq!(engine.composition().text(), "kaifazhe");
    assert_eq!(sent.lock().unwrap()[0].text, "开发");
    reply(&sent, &replies, Some("develop"));
    assert!(engine.poll_prediction().is_none());
    assert_eq!(engine.poll_translation_commit().as_deref(), Some("develop"));
    assert_eq!(engine.composition().text(), "zhe");
    assert!(engine.poll_translation_commit().is_none());
}

#[test]
fn sentence_and_generated_candidates_are_translated_whole() {
    for kind in [CandidateKind::Sentence, CandidateKind::Generated] {
        let (mut engine, sent, replies) = setup();
        let mut whole = candidate(&engine, "开发者");
        whole.kind = kind;
        assert!(engine.commit_translation(&whole, 1).is_none());
        assert_eq!(sent.lock().unwrap()[0].text, "开发者");
        reply(&sent, &replies, Some("developer"));
        assert_eq!(
            engine.poll_translation_commit().as_deref(),
            Some("developer")
        );
        assert!(engine.composition().is_empty());
    }
}

#[test]
fn existing_first_sense_does_not_trigger_fallback_for_missing_second() {
    let (mut engine, sent, _) = setup();
    let mut word = candidate(&engine, "开发");
    word.translation = Some(Translation::new(
        Language::English,
        vec![Sense {
            text: "develop".to_owned(),
            part_of_speech: None,
            reading: None,
            fresh: false,
        }],
    ));
    assert!(engine.commit_translation(&word, 1).is_none());
    assert!(sent.lock().unwrap().is_empty());
    assert_eq!(
        engine.commit_translation(&word, 0).as_deref(),
        Some("develop")
    );
}

#[test]
fn off_private_and_disabled_channel_do_not_submit() {
    for variant in 0..3 {
        let (mut engine, sent, _) = setup();
        match variant {
            0 => engine.set_translation_commit_fallback(false),
            1 => engine.set_private(true),
            _ => engine.set_selection_translator(Box::new(crate::NoSelectionTranslator)),
        }
        let word = candidate(&engine, "开发");
        assert!(engine.commit_translation(&word, 0).is_none());
        assert!(sent.lock().unwrap().is_empty());
        assert_eq!(engine.composition().text(), "kaifazhe");
    }
}

#[test]
fn failed_translation_preserves_input_and_does_not_commit_source() {
    for result in [None, Some("  ")] {
        let (mut engine, sent, replies) = setup();
        let word = candidate(&engine, "开发");
        engine.commit_translation(&word, 0);
        reply(&sent, &replies, result);
        assert!(engine.poll_translation_commit().is_none());
        assert!(!engine.translation_commit_pending());
        assert_eq!(engine.composition().text(), "kaifazhe");
    }
}

#[test]
fn edited_cancelled_private_and_changed_channel_results_never_commit() {
    for variant in 0..5 {
        let (mut engine, sent, replies) = setup();
        let word = candidate(&engine, "开发");
        engine.commit_translation(&word, 0);
        reply(&sent, &replies, Some("stale"));
        match variant {
            0 => engine.push('a'),
            1 => engine.cancel_translation_commit(),
            2 => engine.set_private(true),
            3 => engine.set_selection_translator(Box::new(crate::NoSelectionTranslator)),
            _ => engine.clear(),
        }
        let before = engine.composition().text().to_owned();
        assert!(engine.poll_translation_commit().is_none());
        assert!(!engine.translation_commit_pending());
        assert_eq!(engine.composition().text(), before);
        assert!(engine.poll_prediction().is_none());
    }
}

#[test]
fn fallback_can_use_the_channel_with_selection_hotkey_disabled() {
    let (mut engine, sent, replies) = setup();
    engine.set_selection_translation_enabled(false);
    assert!(!engine.translation_enabled());
    assert!(engine.request_translation("selection").is_none());
    let word = candidate(&engine, "开发");
    engine.commit_translation(&word, 0);
    reply(&sent, &replies, Some("develop"));
    assert_eq!(engine.poll_translation_commit().as_deref(), Some("develop"));
}

#[test]
fn buffer_translation_consumes_whole_english_sentence_even_with_cursor_in_middle() {
    let (mut engine, sent, replies) = setup();
    engine.set_english_mode(true);
    engine.set_input("hello world, how are you?");
    engine.move_cursor_home();
    engine.move_cursor_right();
    assert!(engine.commit_buffer_translation().is_none());
    assert_eq!(sent.lock().unwrap()[0].text, "hello world, how are you?");
    assert_eq!(sent.lock().unwrap()[0].target_language, "zh");
    reply(&sent, &replies, Some("你好世界，你好吗？"));
    assert_eq!(
        engine.poll_translation_commit().as_deref(),
        Some("你好世界，你好吗？")
    );
    assert!(engine.composition().is_empty());
}

#[test]
fn buffer_translation_failure_and_edits_preserve_input() {
    for edited in [false, true] {
        let (mut engine, sent, replies) = setup();
        engine.set_english_mode(true);
        engine.set_input("hello world");
        engine.commit_buffer_translation();
        if edited {
            engine.push('!');
        }
        let before = engine.composition().text().to_owned();
        reply(
            &sent,
            &replies,
            if edited { Some("过期译文") } else { None },
        );
        assert!(engine.poll_translation_commit().is_none());
        assert_eq!(engine.composition().text(), before);
        assert!(!engine.translation_commit_pending());
    }
}
