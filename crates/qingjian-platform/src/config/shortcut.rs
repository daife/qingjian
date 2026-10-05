use qingjian_core::ModeKeys;
use serde::{Deserialize, Serialize};

use super::commit_action::CommitAction;
use super::key_combo::KeyCombo;
use super::modifiers::Modifiers;
use super::switch_key::SwitchKeys;

/// 配置文件 `[shortcut]` 分节：前缀模式键（Core 的 [`ModeKeys`]）加壳层的修饰键组合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShortcutConfig {
    /// 表达式 / 问字模式键，键名与以前一样直接在分节下（`expression` / `question`）。
    #[serde(flatten)]
    pub mode: ModeKeys,

    /// 中 / 英切换键（Windows 用），可多选：`["shift", "control", "ctrl+alt+space"]`。详见 [`SwitchKeys`]。
    pub switch_mode: SwitchKeys,

    /// Windows：回车原样上屏（注音模式为选词）在短按松开时触发，固定键不可重绑。
    pub enter_on_release: bool,

    /// Windows：空格选词（注音待补声调时输入一声）在短按松开时触发，固定键不可重绑。
    pub space_on_release: bool,

    /// Windows：长按回车的动作，关闭时不执行。
    pub enter_hold: CommitAction,

    /// Windows：长按空格的动作，默认上屏高亮候选的首个译词。
    pub space_hold: CommitAction,

    /// 短按的最长按住时间（毫秒），超过时松开不触发。
    pub short_press_ms: u64,

    /// 数字键配这些修饰键：上屏候选的第一个译词。
    pub translation: Modifiers,

    /// 数字键配这些修饰键：上屏候选的第二个译词（候选右侧有两个译词时）。
    pub translation_second: Modifiers,

    /// 把应用里选中的文字译成学习语言（需要云服务开着）。
    pub translate_selection: KeyCombo,

    /// 数字键配这些修饰键：删掉候选（用户词整个删掉，词库词清掉对它的学习）。
    pub delete_candidate: Modifiers,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        // Windows 上 Alt+数字被系统当菜单快捷键截走（TSF 收不到），译词键缺省用 Ctrl；macOS 用 Option。
        #[cfg(windows)]
        let (translation, translation_second) = (Modifiers::CONTROL, Modifiers::SHIFT_CONTROL);
        #[cfg(not(windows))]
        let (translation, translation_second) = (Modifiers::OPTION, Modifiers::SHIFT_OPTION);
        Self {
            enter_on_release: true,
            space_on_release: true,
            enter_hold: CommitAction::None,
            space_hold: CommitAction::FirstTranslation,
            short_press_ms: 500,
            mode: ModeKeys::default(),
            switch_mode: SwitchKeys::default(),
            translation,
            translation_second,
            translate_selection: KeyCombo::TRANSLATE_DEFAULT,
            delete_candidate: Modifiers::SHIFT,
        }
    }
}

impl ShortcutConfig {
    /// 启用长按动作就强制短按松开触发，避免按下时先把候选消耗掉。
    pub fn enter_release(&self) -> bool {
        self.enter_on_release || self.enter_hold.enabled()
    }

    pub fn space_release(&self) -> bool {
        self.space_on_release || self.space_hold.enabled()
    }

    /// 删候选的修饰键；空组合表示关闭，启用时与译词键撞了才退回缺省。
    pub fn delete_keys(&self) -> Modifiers {
        let (first, second) = self.translation_keys();
        if !self.delete_candidate.is_empty()
            && (self.delete_candidate == first || self.delete_candidate == second)
        {
            Self::default().delete_candidate
        } else {
            self.delete_candidate
        }
    }

    /// 两组译词修饰键；空组合表示关闭，两组启用且相同时退回缺省。
    pub fn translation_keys(&self) -> (Modifiers, Modifiers) {
        if !self.translation.is_empty() && self.translation == self.translation_second {
            let default = Self::default();
            (default.translation, default.translation_second)
        } else {
            (self.translation, self.translation_second)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_commit_keys_default_to_release_and_round_trip_settings() {
        let old: ShortcutConfig = toml::from_str("").unwrap();
        assert!(old.enter_on_release && old.space_on_release);
        assert_eq!(old.space_hold, CommitAction::FirstTranslation);
        assert_eq!(old.short_press_ms, 500);
        let config: ShortcutConfig = toml::from_str(
            "enter_on_release = true\nspace_on_release = true\nshort_press_ms = 750\n",
        )
        .unwrap();
        assert!(config.enter_on_release && config.space_on_release);
        assert_eq!(config.short_press_ms, 750);
        assert_eq!(
            toml::from_str::<ShortcutConfig>(&toml::to_string(&config).unwrap()).unwrap(),
            config
        );
    }

    #[test]
    fn hold_actions_force_release_even_when_the_file_requests_press() {
        let config: ShortcutConfig = toml::from_str(
            "enter_on_release = false\nspace_on_release = false\nenter_hold = \"raw\"\nspace_hold = \"first_translation\"\n"
        ).unwrap();
        assert!(config.enter_release() && config.space_release());
        let off: ShortcutConfig = toml::from_str(
            "enter_on_release = false\nspace_on_release = false\nenter_hold = \"none\"\nspace_hold = \"none\"\n"
        ).unwrap();
        assert!(!off.enter_release() && !off.space_release());
    }

    #[test]
    fn old_files_without_modifier_keys_still_parse_and_get_defaults() {
        // 缺省值分平台（Windows 用 Ctrl 系、其余用 Option 系），断言跟着平台的 Default 走
        let default = ShortcutConfig::default();
        let parsed: ShortcutConfig = toml::from_str("expression = \"i\"\n").unwrap();
        assert_eq!(parsed.mode.expression, 'i');
        assert_eq!(parsed.switch_mode, SwitchKeys::default());
        assert_eq!(
            parsed.translation_keys(),
            (default.translation, default.translation_second)
        );
        let same: ShortcutConfig =
            toml::from_str("translation = \"option\"\ntranslation_second = \"option\"\n").unwrap();
        assert_eq!(
            same.translation_keys(),
            (default.translation, default.translation_second)
        );
        let swapped: ShortcutConfig =
            toml::from_str("translation = \"control+option\"\ntranslation_second = \"option\"\n")
                .unwrap();
        assert_eq!(swapped.translation_keys().1, Modifiers::OPTION);
    }

    #[test]
    fn delete_keys_fall_back_when_clashing_with_translation_keys() {
        let default = ShortcutConfig::default();
        let parsed: ShortcutConfig = toml::from_str("").unwrap();
        assert_eq!(parsed.delete_keys(), default.delete_candidate);
        // 与平台缺省的译词键撞上才算「冲突」，两边平台都成立
        let clash: ShortcutConfig = toml::from_str(&format!(
            "delete_candidate = \"{}\"\n",
            default.translation.key()
        ))
        .unwrap();
        assert_eq!(clash.delete_keys(), default.delete_candidate);
        // 不与任何一组译词键冲突的修饰键：平台上取一个，断言它原样生效
        let free = [Modifiers::OPTION, Modifiers::CONTROL, Modifiers::SHIFT]
            .into_iter()
            .find(|m| *m != default.translation && *m != default.translation_second)
            .unwrap();
        let custom: ShortcutConfig =
            toml::from_str(&format!("delete_candidate = \"{}\"\n", free.key())).unwrap();
        assert_eq!(custom.delete_keys(), free);
    }

    #[test]
    fn disabled_shortcuts_round_trip_without_restoring_defaults() {
        let config: ShortcutConfig = toml::from_str(
            r#"expression = "none"
question = "none"
translation = "none"
translation_second = "none"
delete_candidate = "none"
translate_selection = "none"
"#,
        )
        .unwrap();
        assert!(config.mode.sanitized().is_valid());
        assert!(!config.mode.sanitized().is_expression("v1+2", false));
        assert!(!config.mode.sanitized().is_question("u4e00", false));
        assert_eq!(
            config.translation_keys(),
            (Modifiers::default(), Modifiers::default())
        );
        assert!(config.delete_keys().is_empty());
        assert!(config.translate_selection.modifiers.is_empty());
        let saved = toml::to_string(&config).unwrap();
        assert_eq!(toml::from_str::<ShortcutConfig>(&saved).unwrap(), config);
    }

    #[test]
    fn disabling_one_translation_preserves_the_other_binding() {
        let config: ShortcutConfig =
            toml::from_str("translation = \"none\"\ntranslation_second = \"control+option\"\n")
                .unwrap();
        assert!(config.translation_keys().0.is_empty());
        assert_eq!(
            config.translation_keys().1,
            "control+option".parse().unwrap()
        );
        assert_eq!(config.delete_keys(), Modifiers::SHIFT);
    }

    #[test]
    fn switch_mode_parses_and_defaults_to_shift() {
        let parsed: ShortcutConfig = toml::from_str("switch_mode = [\"ctrl\"]\n").unwrap();
        assert!(parsed.switch_mode.control && !parsed.switch_mode.shift);
        let off: ShortcutConfig = toml::from_str("switch_mode = \"none\"\n").unwrap();
        assert_eq!(off.switch_mode, crate::SwitchKeys::NONE);
        let missing: ShortcutConfig = toml::from_str("").unwrap();
        assert_eq!(missing.switch_mode, SwitchKeys::default());
    }
}
