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
