//! 云端猜词只在本轮组句保留，不跨上屏、清空、会话和隐私边界。

use super::super::{EchoPredictor, cloud, engine};
use crate::{EngineSession, Prediction};
use std::{cell::RefCell, rc::Rc};

#[test]
fn history_is_carried_during_editing_and_reset_at_composition_boundaries() {
    let submitted = Rc::new(RefCell::new(Vec::new()));
    let mut engine = engine().with_predictor(Box::new(EchoPredictor {
        submitted: submitted.clone(),
        replies: vec![Prediction {
            sequence: 1,
            words: vec![cloud("开发", &["kai", "fa"])],
            sentence: None,
        }],
        sentence: true,
    }));
    engine.set_input("kaifa");
    engine.request_prediction(None, &[]).unwrap();
    engine.poll_prediction().unwrap();
    engine.push('z');
    engine.request_prediction(None, &[]).unwrap();
    assert_eq!(submitted.borrow().last().unwrap().candidates, ["开发"]);
    engine.clear();
    engine.set_input("kaifa");
    engine.request_prediction(None, &[]).unwrap();
    assert!(submitted.borrow().last().unwrap().candidates.is_empty());

    engine.prediction_history.push("开发".into());
    let query = engine.query().unwrap();
    engine.commit(&query.candidates.items[0]);
    assert!(engine.prediction_history.is_empty());
    engine.prediction_history.push("开发".into());
    engine.swap_session(&mut EngineSession::default());
    assert!(engine.prediction_history.is_empty());
    engine.prediction_history.push("开发".into());
    engine.set_private(true);
    assert!(engine.prediction_history.is_empty());
}
