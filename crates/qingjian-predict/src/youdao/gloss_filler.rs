//! 逐词请求有道翻译，复用释义清洗规则；只保存成功的短译词。

use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, Sender};

use qingjian_core::{FilledGloss, GlossFiller, Language, Sense, Translation};

use super::client::YoudaoClient;
use crate::{PredictConfig, PredictError};

pub(crate) struct YoudaoGlossFiller {
    requests: Sender<(Language, String)>,

    responses: Receiver<FilledGloss>,
}

impl YoudaoGlossFiller {
    pub fn new(config: &PredictConfig) -> Result<Self, PredictError> {
        let client = YoudaoClient::new(config)?;
        let (requests, incoming) = mpsc::channel::<(Language, String)>();
        let (outgoing, responses) = mpsc::channel();
        std::thread::Builder::new()
            .name("qingjian-youdao-gloss".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        tracing::error!(%error, "有道释义线程启动失败");
                        return;
                    }
                };
                let mut asked = HashSet::new();
                while let Ok((language, word)) = incoming.recv() {
                    if language == Language::Chinese || !asked.insert((language, word.clone())) {
                        continue;
                    }
                    match runtime.block_on(client.translate(&word, "zh-CHS", language.code())) {
                        Ok(text) => {
                            if let Some(filled) = filled_gloss(&word, language, &text)
                                && outgoing.send(filled).is_err()
                            {
                                break;
                            }
                        }
                        Err(error) => tracing::warn!(%error, "有道释义补全失败"),
                    }
                }
            })?;
        Ok(Self {
            requests,
            responses,
        })
    }
}

impl GlossFiller for YoudaoGlossFiller {
    fn request(&mut self, language: Language, word: &str) {
        let _ = self.requests.send((language, word.to_owned()));
    }

    fn poll(&mut self) -> Vec<FilledGloss> {
        self.responses.try_iter().collect()
    }
}

pub(super) fn filled_gloss(word: &str, language: Language, text: &str) -> Option<FilledGloss> {
    if language == Language::Chinese {
        return None;
    }
    let text = crate::gloss::clean_text(text, language)?;
    Some(FilledGloss {
        word: word.to_owned(),
        translation: Translation::new(
            language,
            vec![Sense {
                part_of_speech: None,
                text,
                reading: None,
                fresh: false,
            }],
        ),
    })
}
