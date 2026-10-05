//! 选区翻译提供方，与联想和词义补全分别注入。

use crate::engine::prediction::{Prediction, PredictionRequest};

pub trait SelectionTranslator: Send {
    /// 提交整段文字翻译，不能阻塞。
    fn submit(&mut self, request: PredictionRequest);

    /// 取回译文，不能阻塞。
    fn poll(&mut self) -> Option<Prediction>;

    fn is_enabled(&self) -> bool {
        true
    }
}
