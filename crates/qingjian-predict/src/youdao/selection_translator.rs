//! 有道选区翻译独立线程，保留最新请求并缓存成功译文。

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};

use qingjian_core::{Prediction, PredictionKind, PredictionRequest, SelectionTranslator};

use super::client::YoudaoClient;
use crate::{PredictConfig, PredictError};

const CACHE_CAPACITY: usize = 64;

pub(crate) struct YoudaoSelectionTranslator {
    requests: Sender<PredictionRequest>,

    responses: Receiver<Prediction>,
}

impl YoudaoSelectionTranslator {
    pub fn new(config: &PredictConfig) -> Result<Self, PredictError> {
        let client = YoudaoClient::new(config)?;
        let (requests, incoming) = mpsc::channel::<PredictionRequest>();
        let (outgoing, responses) = mpsc::channel();
        std::thread::Builder::new()
            .name("qingjian-youdao-translation".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        tracing::error!(%error, "有道选区翻译线程启动失败");
                        return;
                    }
                };
                let mut cache: HashMap<(String, String), String> = HashMap::new();
                while let Ok(mut request) = incoming.recv() {
                    for newer in incoming.try_iter() {
                        request = newer;
                    }
                    let key = (request.text.clone(), request.target_language.clone());
                    let text = if let Some(text) = cache.get(&key) {
                        text.clone()
                    } else {
                        match runtime.block_on(client.translate(
                            &request.text,
                            "auto",
                            &request.target_language,
                        )) {
                            Ok(text) => {
                                if cache.len() >= CACHE_CAPACITY {
                                    cache.clear();
                                }
                                cache.insert(key, text.clone());
                                text
                            }
                            Err(error) => {
                                tracing::warn!(%error, "有道选区翻译失败");
                                continue;
                            }
                        }
                    };
                    if outgoing
                        .send(Prediction {
                            sequence: request.sequence,
                            words: Vec::new(),
                            sentence: Some(text),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })?;
        Ok(Self {
            requests,
            responses,
        })
    }
}

impl SelectionTranslator for YoudaoSelectionTranslator {
    fn submit(&mut self, request: PredictionRequest) {
        if request.kind == PredictionKind::Translate {
            let _ = self.requests.send(request);
        }
    }

    fn poll(&mut self) -> Option<Prediction> {
        self.responses.try_recv().ok()
    }
}
