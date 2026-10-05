//! 选区翻译的大模型渠道，共用云服务配置但独立于预测任务。

use crate::{CloudPredictor, PredictConfig, PredictError};
use qingjian_core::{Prediction, PredictionRequest, Predictor, SelectionTranslator};

pub struct CloudSelectionTranslator {
    worker: CloudPredictor,
}

impl CloudSelectionTranslator {
    pub fn new(config: &PredictConfig) -> Result<Self, PredictError> {
        Ok(Self {
            worker: CloudPredictor::new(config)?,
        })
    }
}

impl SelectionTranslator for CloudSelectionTranslator {
    fn submit(&mut self, request: PredictionRequest) {
        self.worker.submit(request);
    }

    fn poll(&mut self) -> Option<Prediction> {
        self.worker.poll()
    }
}
