//! 中英文整句补全开关的独立性。

use crate::engine::{Prediction, PredictionPolicy, PredictionRequest, Predictor};

struct CompletionPredictor(PredictionPolicy);

impl Predictor for CompletionPredictor {
    fn policy(&self) -> PredictionPolicy {
        self.0
    }
    fn submit(&mut self, _: PredictionRequest) {}
    fn poll(&mut self) -> Option<Prediction> {
        None
    }
}

#[test]
fn chinese_and_english_completion_switches_are_independent() {
    for chinese in [false, true] {
        for english in [false, true] {
            let predictor = CompletionPredictor(PredictionPolicy {
                sentence: chinese,
                english_sentence: english,
                ..PredictionPolicy::default()
            });
            let mut engine = super::engine().with_predictor(Box::new(predictor));
            engine.set_english_mode(true);
            engine.set_input("hello");
            assert_eq!(engine.request_prediction(None, &[]).is_some(), english);
            engine.set_english_mode(false);
            engine.set_input("nihao");
            assert!(
                engine.request_prediction(None, &[]).is_some(),
                "中文候选词仍可请求"
            );
            assert_eq!(engine.prediction_policy().sentence, chinese);
        }
    }
}

pub(super) struct Recorder {
    pub(super) policy: PredictionPolicy,

    pub(super) submitted: std::sync::Arc<std::sync::Mutex<Vec<PredictionRequest>>>,

    pub(super) reply: Option<Prediction>,
}

impl Predictor for Recorder {
    fn policy(&self) -> PredictionPolicy {
        self.policy
    }

    fn submit(&mut self, request: PredictionRequest) {
        self.submitted.lock().unwrap().push(request);
    }

    fn poll(&mut self) -> Option<Prediction> {
        self.reply.take()
    }
}

#[test]
fn word_and_sentence_switches_control_actual_requests_for_each_language() {
    for english in [false, true] {
        for words in [false, true] {
            for sentence in [false, true] {
                for slots in [0, 2] {
                    let submitted = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
                    let policy = PredictionPolicy {
                        chinese_words: !english && words,
                        english_words: english && words,
                        sentence: !english && sentence,
                        english_sentence: english && sentence,
                        slots,
                        ..PredictionPolicy::default()
                    };
                    let mut engine = super::engine().with_predictor(Box::new(Recorder {
                        policy,
                        submitted: submitted.clone(),
                        reply: None,
                    }));
                    engine.set_english_mode(english);
                    engine.set_input(if english { "prog" } else { "kaifa" });
                    let want_words = words && slots > 0;
                    assert_eq!(
                        engine.request_prediction(None, &[]).is_some(),
                        want_words || sentence
                    );
                    let submitted = submitted.lock().unwrap();
                    if let Some(request) = submitted.first() {
                        assert_eq!(request.max_items > 0, want_words);
                        assert_eq!(request.want_sentence, sentence);
                    } else {
                        assert!(!want_words && !sentence);
                    }
                }
            }
        }
    }
}

#[test]
fn abbreviated_chinese_and_multiword_english_do_not_request_word_only_tasks() {
    for (english, input) in [(false, "kf"), (true, "hello world")] {
        let mut engine =
            super::engine().with_predictor(Box::new(CompletionPredictor(PredictionPolicy {
                chinese_words: true,
                english_words: true,
                sentence: false,
                english_sentence: false,
                ..PredictionPolicy::default()
            })));
        engine.set_english_mode(english);
        engine.set_input(input);
        assert!(engine.request_prediction(None, &[]).is_none());
    }
}
