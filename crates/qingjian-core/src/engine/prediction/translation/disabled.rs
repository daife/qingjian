//! 关闭选区翻译时的空实现。

use super::SelectionTranslator;
use crate::engine::prediction::{Prediction, PredictionRequest};

#[derive(Debug, Default)]
pub struct NoSelectionTranslator;

impl SelectionTranslator for NoSelectionTranslator {
    fn submit(&mut self, _: PredictionRequest) {}

    fn poll(&mut self) -> Option<Prediction> {
        None
    }

    fn is_enabled(&self) -> bool {
        false
    }
}
