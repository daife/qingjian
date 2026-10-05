//! 选区翻译独立于预测，切换渠道作废旧回复。

use crate::{Prediction, PredictionRequest, SelectionTranslator};
use std::sync::{Arc, Mutex};

struct Translator {
    submitted: Arc<Mutex<Vec<PredictionRequest>>>,

    reply: Option<Prediction>,
}

impl SelectionTranslator for Translator {
    fn submit(&mut self, request: PredictionRequest) {
        self.submitted.lock().unwrap().push(request);
    }

    fn poll(&mut self) -> Option<Prediction> {
        self.reply.take()
    }
}

#[test]
fn selection_translation_works_without_predictor_and_stops_when_removed_or_private() {
    let submitted = Arc::new(Mutex::new(Vec::new()));
    let mut engine = super::engine().with_selection_translator(Box::new(Translator {
        submitted: submitted.clone(),
        reply: None,
    }));
    engine.set_input("kaifa");
    assert!(engine.request_prediction(None, &[]).is_none());
    assert!(engine.translation_enabled());
    let sequence = engine.request_translation("hello").unwrap();
    assert_eq!(submitted.lock().unwrap().len(), 1);
    engine.selection_translator = Box::new(Translator {
        submitted: submitted.clone(),
        reply: Some(Prediction {
            sequence,
            words: Vec::new(),
            sentence: Some("你好".into()),
        }),
    });
    assert_eq!(
        engine.poll_prediction().unwrap().sentence.as_deref(),
        Some("你好")
    );
    engine.set_private(true);
    assert!(!engine.translation_enabled());
    assert!(engine.request_translation("private").is_none());
    engine.set_private(false);
    engine.set_selection_translator(Box::new(crate::NoSelectionTranslator));
    assert!(!engine.translation_enabled());
    assert!(engine.request_translation("disabled").is_none());
    assert_eq!(submitted.lock().unwrap().len(), 1);
}

#[test]
fn changing_translation_channel_discards_previous_sequence() {
    let mut engine = super::engine().with_selection_translator(Box::new(Translator {
        submitted: Default::default(),
        reply: None,
    }));
    let sequence = engine.request_translation("hello").unwrap();
    engine.set_selection_translator(Box::new(Translator {
        submitted: Default::default(),
        reply: Some(Prediction {
            sequence,
            words: Vec::new(),
            sentence: Some("旧结果".into()),
        }),
    }));
    assert!(engine.poll_prediction().is_none());
}
