//! 无译词上屏的异步托底：复用选区翻译渠道，确认输入状态未变后再消耗候选。

use std::time::{Duration, Instant};

use crate::engine::input_log::InputSource;
use crate::{Candidate, Engine};

pub(in crate::engine) struct TranslationCommit {
    candidate: Candidate,

    sequence: u64,

    input: String,

    cursor: usize,

    since: Instant,
}

impl Engine {
    pub fn set_translation_commit_fallback(&mut self, enabled: bool) {
        self.cancel_translation_commit();
        self.translation_commit_fallback = enabled;
    }

    pub fn translation_commit_fallback_enabled(&self) -> bool {
        self.translation_commit_fallback && !self.private && self.selection_translator.is_enabled()
    }

    pub fn translation_commit_pending(&self) -> bool {
        self.translation_commit.is_some()
    }

    pub fn translation_commit_sequence(&self) -> Option<u64> {
        self.translation_commit
            .as_ref()
            .map(|pending| pending.sequence)
    }

    pub fn cancel_translation_commit(&mut self) {
        if self.translation_commit.take().is_some() {
            self.prediction_sequence += 1;
        }
    }

    pub(super) fn request_translation_commit(&mut self, candidate: &Candidate) {
        let has_translation = candidate.translation.as_ref().is_some_and(|translation| {
            translation
                .senses()
                .iter()
                .any(|sense| !sense.text.trim().is_empty())
        });
        if !self.translation_commit_fallback || has_translation || self.composition.is_empty() {
            return;
        }
        self.cancel_translation_commit();
        let Some(sequence) = self.submit_translation(&candidate.text) else {
            return;
        };
        self.translation_commit = Some(TranslationCommit {
            candidate: candidate.clone(),
            sequence,
            input: self.composition.text().to_owned(),
            cursor: self.composition.cursor(),
            since: Instant::now(),
        });
    }

    /// 译文到达后按原候选消耗输入；失败、过期或状态变化时保留输入。
    pub fn poll_translation_commit(&mut self) -> Option<String> {
        let pending = self.translation_commit.as_ref()?;
        if self.private
            || !self.selection_translator.is_enabled()
            || pending.sequence != self.prediction_sequence
            || pending.input != self.composition.text()
            || pending.cursor != self.composition.cursor()
            || pending.since.elapsed() > Duration::from_secs(90)
        {
            self.cancel_translation_commit();
            return None;
        }
        while let Some(reply) = self.selection_translator.poll() {
            if reply.sequence != self.prediction_sequence {
                continue;
            }
            let pending = self.translation_commit.take()?;
            let text = reply.sentence.filter(|text| !text.trim().is_empty())?;
            self.commit_with(&pending.candidate, InputSource::Translation, None);
            return Some(text);
        }
        None
    }
}
