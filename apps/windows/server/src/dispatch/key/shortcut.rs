//! 组句中「修饰键 + 数字」的快捷键：上屏译词、删候选。与 macOS 壳对齐。

use qingjian_core::{Candidate, CandidateList};
use qingjian_platform::CommitAction;
use qingjian_platform::protocol::{KeyEvent, KeyModifiers, TRANSLATION_COMMIT_SINCE};

use super::Effect;
use crate::dispatch::Router;

impl Router {
    /// 长按动作按 Server 的当前配置解析；没有候选 / 译词时保留缓冲区。
    pub(super) fn apply_hold(&mut self, event: &KeyEvent) -> Effect {
        if event.modifiers.has_command_key() || self.engine.composition().is_empty() {
            return Effect::Navigated;
        }
        let action = match event.virtual_key {
            0x0d => self.config.enter_hold,
            0x20 => self.config.space_hold,
            _ => return Effect::Navigated,
        };
        match action {
            CommitAction::InsertSpace => {
                self.engine.clear_aux();
                self.engine.push(' ');
                Effect::Changed(None)
            }
            CommitAction::None => Effect::Navigated,
            CommitAction::Candidate => match self.commit_index(self.highlight) {
                Some(text) => Effect::Changed(Some(text)),
                None => Effect::Navigated,
            },
            CommitAction::Raw => Effect::Changed(Some(self.engine.take_raw())),
            CommitAction::FirstTranslation => {
                let digit = self.highlight % self.config.page_size + 1;
                self.commit_translation_on_page(digit, 0)
            }
        }
    }

    /// 配到哪组就上屏第一 / 第二个译词或删候选；哪组都不是返回 `None`，按普通键处理。
    pub(super) fn apply_digit_shortcut(
        &mut self,
        digit: usize,
        chord: KeyModifiers,
    ) -> Option<Effect> {
        if chord == KeyModifiers::default() {
            return None;
        }
        let (first, second) = self.config.translation_keys;
        let effect = if chord == first {
            self.commit_translation_on_page(digit, 0)
        } else if chord == second {
            self.commit_translation_on_page(digit, 1)
        } else if chord == self.config.delete_keys {
            self.forget_on_page(digit)
        } else {
            return None;
        };
        Some(effect)
    }

    /// 上屏第 `digit` 个候选的第 `sense` 条译文；没有那条译文就吞掉按键不动。
    fn commit_translation_on_page(&mut self, digit: usize, sense: usize) -> Effect {
        let Some(candidate) = self.annotated_candidate_on_page(digit) else {
            tracing::debug!(digit, "这一格没有候选");
            return Effect::Navigated;
        };
        if self.focused.is_some_and(|session| {
            self.sessions
                .get(&session)
                .is_some_and(|info| info.protocol < TRANSLATION_COMMIT_SINCE)
        }) && self.engine.translation_commit_fallback_enabled()
            && !candidate.translation.as_ref().is_some_and(|translation| {
                translation
                    .senses()
                    .iter()
                    .any(|sense| !sense.text.trim().is_empty())
            })
        {
            self.notice = Some("请重新打开当前应用以使用译词上屏托底".to_owned());
            return Effect::Navigated;
        }
        match self.engine.commit_translation(&candidate, sense) {
            Some(text) => Effect::Changed(Some(text)),
            None => {
                if self.engine.translation_commit_pending() {
                    self.notice = Some("正在翻译，继续输入可取消…".to_owned());
                }
                tracing::debug!(digit, sense, "这个候选没有这条译文");
                Effect::Navigated
            }
        }
    }

    /// 删第 `digit` 个候选（用户词整删、词库词清学习记录），提示随下一帧下发。
    fn forget_on_page(&mut self, digit: usize) -> Effect {
        let Some(candidate) = self.candidate_on_page(digit) else {
            tracing::debug!(digit, "这一格没有候选，没什么可删");
            return Effect::Navigated;
        };
        let forgotten = self.engine.forget(&candidate);
        let text = &candidate.text;
        let message = if forgotten.user_word {
            format!("已删除用户词「{text}」")
        } else if forgotten.learning {
            format!("已忘掉对「{text}」的学习记录")
        } else {
            format!("「{text}」是词库里的词，也没有学习记录，没什么可删")
        };
        tracing::info!(%message);
        self.notice = Some(message);
        Effect::Changed(None)
    }

    /// 当前页第 `digit` 个候选（1 起）。
    fn candidate_on_page(&self, digit: usize) -> Option<Candidate> {
        let page_size = self.config.page_size;
        let page = self.highlight / page_size;
        let index = page * page_size + digit.checked_sub(1)?;
        self.layout_candidate(index)
    }

    /// 同上，补上译文（布局里存的是查询原样，译文画页时才补）。
    fn annotated_candidate_on_page(&self, digit: usize) -> Option<Candidate> {
        let candidate = self.candidate_on_page(digit)?;
        let mut list = CandidateList {
            items: vec![candidate],
        };
        self.engine.annotate(&mut list);
        list.items.pop()
    }
}
