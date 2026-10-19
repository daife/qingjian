//! 「快捷键」页：翻页键、模式键，译词 / 删候选 / 翻译选中文字的修饰键。
//! 翻译选中文字只改修饰键，字母键固定用配置里当前的；要换字母直接改 `config.toml`。

use qingjian_platform::{CommitAction, KeyCombo, Modifiers};
use windows_reactor::{
    Callback, ChildrenControl, ComboBox, NumberBox, StackPanel, ToggleSwitch, View, ViewContext,
};

use crate::panel::controls::{field, index_of, page};
use crate::panel::{Message, Settings};

/// 翻页键对：界面名 + 配置写法。
pub(crate) const PAGE_KEYS: [(&str, &str); 4] = [
    ("方括号 [ ]", "[]"),
    ("逗号句号 , .", ",."),
    ("减号等号 - =", "-="),
    ("关闭", "none"),
];

/// 可当模式键的字母（与 Core `ModeKeys::OPTIONS` 一致，none 表示关闭）。
pub(crate) const MODE_KEYS: [&str; 4] = ["v", "u", "i", "none"];

/// 修饰键预设：界面名 + 配置写法。
pub(crate) const MODIFIERS: [(&str, &str); 7] = [
    ("Ctrl", "ctrl"),
    ("Alt", "alt"),
    ("Shift", "shift"),
    ("Ctrl + Shift", "shift+ctrl"),
    ("Ctrl + Alt", "ctrl+alt"),
    ("Alt + Shift", "shift+alt"),
    ("关闭", "none"),
];

fn mode_combo(current: char, callback: Callback<Option<usize>>) -> ComboBox {
    let selected = MODE_KEYS
        .iter()
        .position(|key| key.starts_with(current) || (current == '\0' && *key == "none"))
        .unwrap_or(0);
    ComboBox::new()
        .items_source(["v", "u", "i", "关闭"])
        .selected_index(selected)
        .on_selection_changed(callback)
}

/// 按解析后相等找当前项，不依赖字符串写法。
fn modifier_combo(current: Modifiers, callback: Callback<Option<usize>>) -> ComboBox {
    let selected = MODIFIERS
        .iter()
        .position(|(_, value)| value.parse::<Modifiers>().ok() == Some(current))
        .unwrap_or(0);
    ComboBox::new()
        .items_source(MODIFIERS.iter().map(|(label, _)| *label))
        .selected_index(selected)
        .on_selection_changed(callback)
}

pub(crate) fn view(settings: &Settings, context: &mut ViewContext<Settings>) -> View {
    let s = &settings.config.shortcut;
    let rows = [
        assist_shortcut("截图 OCR", s.screenshot_ocr, false, context),
        assist_shortcut("通用语音识别", s.speech_recognition, true, context),
        field(
            "长按空格（Space）",
            "按键固定，超过阈值后松开执行。启用后空格选词改为短按松开触发；默认在输入内容的光标位置添加一个空格，继续输入整句；未在输入时按键仍交给应用。",
            ComboBox::new()
                .items_source(CommitAction::ALL.iter().map(|action| action.label()))
                .selected_index(
                    CommitAction::ALL
                        .iter()
                        .position(|action| *action == s.space_hold),
                )
                .on_selection_changed(context.callback(Message::SpaceHold)),
        ),
        field(
            "长按回车（Enter）",
            "按键固定，超过阈值后松开执行。启用后回车原样上屏改为短按松开触发（注音模式为选词）。默认上屏高亮候选的首个译词，没有译词时保留输入。",
            ComboBox::new()
                .items_source(CommitAction::ALL.iter().map(|action| action.label()))
                .selected_index(
                    CommitAction::ALL
                        .iter()
                        .position(|action| *action == s.enter_hold),
                )
                .on_selection_changed(context.callback(Message::EnterHold)),
        ),
        field(
            "原样上屏（Enter）：短按松开时触发",
            "回车将正在输入的拼音、编码或英文原样上屏，不选择候选。注音模式下回车上屏高亮候选，Shift + Enter 原样上屏注音。开启时短按松开触发，关闭时按下触发；按住达到阈值立即执行一次长按动作，无需松开；长按关闭则不触发。",
            ToggleSwitch::new()
                .is_on(s.enter_release())
                .is_enabled(!s.enter_hold.enabled())
                .on_toggled(context.callback(Message::EnterOnRelease)),
        ),
        field(
            "候选上屏（Space）：短按松开时触发",
            "空格上屏当前高亮候选；无候选时原样上屏，辅码筛选为空时保留输入。英文选词后保留空格，英文直输段原样上屏并加空格；注音待补声调时输入一声。开启时短按松开触发，关闭时按下触发；按住达到阈值立即执行一次长按动作，无需松开；长按关闭则不触发。",
            ToggleSwitch::new()
                .is_on(s.space_release())
                .is_enabled(!s.space_hold.enabled())
                .on_toggled(context.callback(Message::SpaceOnRelease)),
        ),
        field(
            "短按 / 长按分界（毫秒）",
            "回车与空格共用，范围 100–2000 毫秒，默认 500。仅影响开启松开触发的按键。",
            NumberBox::new()
                .minimum(100.0)
                .maximum(2000.0)
                .value(s.short_press_ms.clamp(100, 2000) as f64)
                .on_value_changed(context.callback(Message::ShortPressMs)),
        ),
        field(
            "翻页键",
            "选「, .」或「- =」时组句中敲对应符号是翻页，不再是上屏加标点。",
            ComboBox::new()
                .items_source(PAGE_KEYS.iter().map(|(label, _)| *label))
                .selected_index(index_of(&PAGE_KEYS, &settings.config.general.page_keys))
                .on_selection_changed(context.callback(Message::PageKeys)),
        ),
        field(
            "表达式模式键",
            "",
            mode_combo(s.mode.expression, context.callback(Message::ModeExpression)),
        ),
        field(
            "问字模式键",
            "这两个字母开头进模式：v1+2 出 3，usangemu 问「三个木」（需要云服务）。两个键不能相同。",
            mode_combo(s.mode.question, context.callback(Message::ModeQuestion)),
        ),
        field(
            "没在输入拼音时敲 ? 也进入问字",
            "开着时 ? 先进问字（中英文模式都行），后面跟字母才是问题，跟其他键时还原成问号；关着问号就是问号。",
            ToggleSwitch::new()
                .is_on(s.mode.question_mark)
                .on_toggled(context.callback(Message::QuestionMark)),
        ),
        field(
            "译词上屏（第一个）",
            "按住修饰键再按候选序号，上屏候选右侧的译词而不是中文。",
            modifier_combo(s.translation, context.callback(Message::Translation)),
        ),
        field(
            "译词上屏（第二个）",
            "候选有两个译词时，这组键上屏后一个。两组不能相同。Ctrl + Shift 是 Windows 切换输入法的热键，同时装着别的输入法时别选它。",
            modifier_combo(
                s.translation_second,
                context.callback(Message::TranslationSecond),
            ),
        ),
        field(
            "删除候选",
            "按住修饰键再按候选序号：自己造的词、云端选过的词整删；词库里的词清掉学习记录，回到原排序。",
            modifier_combo(
                s.delete_candidate,
                context.callback(Message::DeleteCandidate),
            ),
        ),
        field(
            "翻译选中文字",
            "选中一段文字后按这组键 + 当前字母（缺省 Ctrl+Alt+T），把它译成学习语言，回车 / 空格替换、Esc 保留原文。需要云服务。这里只改修饰键，字母固定用当前的。",
            modifier_combo(
                s.translate_selection.modifiers,
                context.callback(Message::TranslateSelection),
            ),
        ),
    ];
    page("快捷键", StackPanel::new().spacing(16.0).children(rows))
}

/// 全局快捷键的全部修饰键组合。
pub(crate) const ASSIST_MODIFIERS: [(&str, &str); 16] = [
    ("Ctrl + Shift", "ctrl+shift"),
    ("Ctrl + Alt", "ctrl+alt"),
    ("Alt + Shift", "alt+shift"),
    ("Ctrl", "ctrl"),
    ("Alt", "alt"),
    ("Shift", "shift"),
    ("Ctrl + Alt + Shift", "ctrl+alt+shift"),
    ("Win", "win"),
    ("Win + Ctrl", "win+ctrl"),
    ("Win + Alt", "win+alt"),
    ("Win + Shift", "win+shift"),
    ("Win + Ctrl + Alt", "win+ctrl+alt"),
    ("Win + Ctrl + Shift", "win+ctrl+shift"),
    ("Win + Alt + Shift", "win+alt+shift"),
    ("Win + Ctrl + Alt + Shift", "win+ctrl+alt+shift"),
    ("关闭", "none"),
];
pub(crate) const ASSIST_KEYS: &str = "abcdefghijklmnopqrstuvwxyz0123456789";

fn assist_shortcut(
    label: &str,
    combo: KeyCombo,
    speech: bool,
    context: &mut ViewContext<Settings>,
) -> View {
    use windows_reactor::Orientation;
    let selected = ASSIST_MODIFIERS
        .iter()
        .position(|(_, value)| value.parse::<Modifiers>().ok() == Some(combo.modifiers));
    field(
        label,
        "全局快捷键；功能开关与渠道在「辅助功能」。已被系统或其他程序占用时会提示，请更换组合。",
        StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(8.0)
            .children((
                ComboBox::new()
                    .items_source(ASSIST_MODIFIERS.iter().map(|(label, _)| *label))
                    .selected_index(selected)
                    .on_selection_changed(
                        context.callback(move |i| Message::AssistModifiers(speech, i)),
                    ),
                ComboBox::new()
                    .items_source(
                        ASSIST_KEYS
                            .chars()
                            .map(|c| c.to_ascii_uppercase().to_string()),
                    )
                    .selected_index(ASSIST_KEYS.find(combo.key))
                    .on_selection_changed(context.callback(move |i| Message::AssistKey(speech, i))),
            )),
    )
}
