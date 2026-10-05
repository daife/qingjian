//! 英文句组的字符收录策略，三个平台共用。

use super::Engine;

impl Engine {
    pub fn set_english_composition(&mut self, space: bool, enter: bool, punctuation: bool) {
        if self.english_compose != [space, enter, punctuation] {
            self.cancel_translation_commit();
            self.english_compose = [space, enter, punctuation];
        }
    }

    /// 英文候选模式下优先收录已配置的字符，不再将其用于选词或翻页。
    pub fn english_composes(&self, c: char) -> bool {
        self.english_mode
            && match c {
                ' ' => self.english_compose[0],
                '\n' => self.english_compose[1],
                c if c.is_ascii_punctuation() => self.english_compose[2],
                _ => false,
            }
    }
}
