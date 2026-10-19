//! Windows 辅助功能：功能开关、识别渠道和语音语言。

use windows_reactor::{ChildrenControl, ComboBox, StackPanel, ToggleSwitch, View, ViewContext};

use crate::panel::controls::{field, note, page};
use crate::panel::{Message, Settings};

pub(crate) fn view(settings: &Settings, context: &mut ViewContext<Settings>) -> View {
    let config = &settings.config.accessibility;
    page("辅助功能", StackPanel::new().spacing(16.0).children([
        field("截图通用 OCR", "拖动框选屏幕文字，松开鼠标开始识别；Esc 或鼠标右键取消。", ToggleSwitch::new().is_on(config.ocr_enabled).on_toggled(context.callback(Message::OcrEnabled))),
        field("OCR 渠道", "", ComboBox::new().items_source(["有道公开接口"]).selected_index((config.ocr_channel == "youdao_public").then_some(0)).on_selection_changed(context.callback(Message::OcrChannel))),
        field("通用语音识别", "使用默认麦克风，空格结束并识别；Esc 或鼠标右键取消。最长 60 秒，到时自动识别。", ToggleSwitch::new().is_on(config.speech_enabled).on_toggled(context.callback(Message::SpeechEnabled))),
        field("语音渠道", "", ComboBox::new().items_source(["有道公开接口"]).selected_index((config.speech_channel == "youdao_public").then_some(0)).on_selection_changed(context.callback(Message::SpeechChannel))),
        field("语音语言", "", ComboBox::new().items_source(["中文", "英文"]).selected_index(match config.speech_language.as_str() { "zh-CHS" => Some(0), "en" => Some(1), _ => None }).on_selection_changed(context.callback(Message::SpeechLanguage))),
        note("快捷键在「快捷键」页配置。识别结果复制到剪贴板；原输入框仍可编辑且未切换目标时，也会尝试直接写入。"),
        note("开启后，选区图片或录音会发送给有道公开网页接口，无需 API 密钥。该接口没有稳定性或配额承诺，遇到限频请稍后手动重试。截图与录音不保存到磁盘。"),
    ]))
}
