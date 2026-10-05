//! 英文云端候选的校验与学习。

use crate::engine::tests::completion::Recorder;
use crate::engine::tests::{FixedTranslator, MemoryFiller, engine};
use crate::{Prediction, PredictionPolicy};

#[test]
fn english_cloud_words_validate_prefix_and_follow_input_case() {
    let words = [
        "programming",
        "progress",
        "wrong",
        "prog",
        "prog ram",
        "程序",
    ]
    .into_iter()
    .map(|text| crate::CloudWord {
        text: text.into(),
        ..crate::CloudWord::default()
    })
    .collect();
    let mut engine = engine().with_predictor(Box::new(Recorder {
        policy: PredictionPolicy {
            english_words: true,
            english_sentence: false,
            ..PredictionPolicy::default()
        },
        submitted: Default::default(),
        reply: Some(Prediction {
            sequence: 1,
            words,
            sentence: Some("unrequested sentence".into()),
        }),
    }));
    engine.set_english_mode(true);
    engine.set_input("PROG");
    engine.request_prediction(None, &[]).unwrap();
    let reply = engine.poll_prediction().unwrap();
    assert_eq!(
        reply
            .words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>(),
        ["PROGRAMMING", "PROGRESS"]
    );
    assert!(reply.sentence.is_none());
}

#[test]
fn english_cloud_words_learn_as_english_and_never_request_chinese_glosses() {
    struct EnglishLearner(std::sync::Arc<std::sync::Mutex<Vec<String>>>);
    impl crate::Learner for EnglishLearner {
        fn weight(&self, _: &str) -> u32 {
            0
        }
        fn record(&mut self, candidate: &crate::Candidate) {
            assert_eq!(candidate.kind, crate::CandidateKind::English);
        }
        fn learn_english(&mut self, text: &str) {
            self.0.lock().unwrap().push(text.into());
        }
        fn learn_word(&mut self, _: &str, _: &[String]) {
            panic!("英文词不能记入拼音用户词");
        }
        fn record_choice(&mut self, _: &str, _: &str) {
            panic!("英文词不能记拼音选词");
        }
        fn forget_english(&mut self, text: &str) -> bool {
            self.0.lock().unwrap().retain(|word| word != text);
            true
        }
    }
    let learned = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let filler = MemoryFiller::default();
    let requested = filler.requested.clone();
    let mut engine = engine()
        .with_learner(Box::new(EnglishLearner(learned.clone())))
        .with_gloss_filler(Box::new(filler))
        .with_translator(Box::new(FixedTranslator));
    engine.set_english_mode(true);
    engine.set_input("prog");
    let candidate = crate::CloudWord {
        text: "programming".into(),
        ..crate::CloudWord::default()
    }
    .into_candidate();
    assert_eq!(engine.commit(&candidate), "programming");
    assert!(engine.composition().is_empty());
    assert_eq!(*learned.lock().unwrap(), ["programming"]);
    assert!(requested.lock().unwrap().is_empty());
    assert!(engine.forget(&candidate).user_word);
    assert!(learned.lock().unwrap().is_empty());
}
