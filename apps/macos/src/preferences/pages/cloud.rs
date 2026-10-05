//! 「云服务」页：本地整句模型开关，云联想开关、云端词格数、接口地址 / 模型 / 密钥、测试连接。

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSButton, NSPopUpButton, NSSecureTextField, NSTextField};
use objc2_foundation::NSString;
use qingjian_platform::Config;

use crate::preferences::controls::{
    button, checkbox, note, row_checkbox, row_control, row_popup, secure_field, select,
    set_checked, text_field,
};
use crate::preferences::layout::{Layout, PAGE_PADDING, ROW_HEIGHT};
use crate::preferences::setting::Setting;
use crate::preferences::target::PreferencesTarget;

/// 云端词槽位弹出菜单的上限（配置文件里可以填更大，菜单只列到这）。
const MAX_CLOUD_SLOTS: usize = 4;

pub struct CloudPage {
    /// 本地整句模型开关。
    local_model: Retained<NSButton>,

    /// 云联想开关。
    enabled: Retained<NSButton>,

    /// 云端词槽位数（0–4）。
    slots: Retained<NSPopUpButton>,

    /// 中文云端候选词联想。
    chinese_words: Retained<NSButton>,

    /// 英文云端候选词联想。
    english_words: Retained<NSButton>,

    /// 缺失释义补全。
    gloss: Retained<NSButton>,

    /// 翻译选中文字。
    translation: Retained<NSButton>,

    /// 释义补全渠道。
    gloss_channel: Retained<NSPopUpButton>,

    /// 选区翻译渠道。
    translation_channel: Retained<NSPopUpButton>,

    /// 中文整句补全开关。
    sentence: Retained<NSButton>,

    /// 英文整句补全开关。
    english_sentence: Retained<NSButton>,

    /// 场景参考开关。
    scenarios_enabled: Retained<NSButton>,

    /// 常用场景关键词。
    scenarios: Retained<NSTextField>,

    /// 接口地址。
    base_url: Retained<NSTextField>,

    /// 模型名。
    model: Retained<NSTextField>,

    /// 密钥输入框，永远不回显已有值。
    api_key: Retained<NSSecureTextField>,

    /// 「测试连接」按钮。
    test: Retained<NSButton>,
}

impl CloudPage {
    pub fn build(layout: &mut Layout, mtm: MainThreadMarker, target: &PreferencesTarget) -> Self {
        let local_model = checkbox(mtm, "本地整句模型", Setting::LocalModelEnabled, target);
        row_checkbox(layout, &local_model);
        note(
            layout,
            mtm,
            "随包的小模型在本机给整句候选重新排序，全程离线；停键后几十毫秒生效。关掉只用词库统计。",
        );
        let enabled = checkbox(mtm, "启用云服务", Setting::CloudEnabled, target);
        row_checkbox(layout, &enabled);
        note(
            layout,
            mtm,
            "开启后组句时会把光标附近的几十个字发给下面的服务，让模型补全整句、联想下文；密码框里绝不发送。菜单栏图标旁会带一个云朵。",
        );
        let slot_titles: Vec<String> = (0..=MAX_CLOUD_SLOTS)
            .map(|n| match n {
                0 => "不要云端候选词".to_owned(),
                n => format!("{n} 格"),
            })
            .collect();
        let slots = row_popup(
            layout,
            mtm,
            "云端词位置",
            &slot_titles,
            Setting::CloudSlots,
            target,
        );
        note(
            layout,
            mtm,
            "云端词到了只补进第一页末尾这几格（比如 2 就是 8、9），前面的本地候选不动；没到就什么都不变，翻页后全是本地候选。",
        );
        let channel_titles: Vec<String> = qingjian_predict::TranslationChannel::ALL
            .iter()
            .map(|c| c.label().to_owned())
            .collect();
        let chinese_words = checkbox(
            mtm,
            "中文云端候选词联想",
            Setting::CloudChineseWords,
            target,
        );
        row_checkbox(layout, &chinese_words);
        let english_words = checkbox(
            mtm,
            "英文云端候选词联想",
            Setting::CloudEnglishWords,
            target,
        );
        row_checkbox(layout, &english_words);
        let gloss = checkbox(mtm, "缺失释义补全", Setting::CloudGloss, target);
        row_checkbox(layout, &gloss);
        let translation = checkbox(mtm, "翻译选中文字", Setting::CloudTranslation, target);
        row_checkbox(layout, &translation);
        let gloss_channel = row_popup(
            layout,
            mtm,
            "释义补全渠道",
            &channel_titles,
            Setting::CloudGlossChannel,
            target,
        );
        let translation_channel = row_popup(
            layout,
            mtm,
            "选区翻译渠道",
            &channel_titles,
            Setting::CloudTranslationChannel,
            target,
        );
        let sentence = checkbox(mtm, "中文整句补全", Setting::CloudSentence, target);
        row_checkbox(layout, &sentence);
        let english_sentence = checkbox(mtm, "英文整句补全", Setting::CloudEnglishSentence, target);
        row_checkbox(layout, &english_sentence);
        let scenarios_enabled = checkbox(
            mtm,
            "联想 & 整句补全场景",
            Setting::CloudScenariosEnabled,
            target,
        );
        row_checkbox(layout, &scenarios_enabled);
        let scenarios = text_field(mtm, Setting::CloudScenarios, target);
        row_control(layout, mtm, "常用场景", &scenarios);
        note(
            layout,
            mtm,
            "自行填写，用逗号或分号分隔，如：日常聊天，我的世界游戏，IT 行业。开启后发送给云服务，供中英文联想与整句补全参考。",
        );
        let base_url = text_field(mtm, Setting::BaseUrl, target);
        row_control(layout, mtm, "接口地址", &base_url);
        let model = text_field(mtm, Setting::Model, target);
        row_control(layout, mtm, "模型", &model);
        let api_key = secure_field(mtm, Setting::ApiKey, target);
        row_control(layout, mtm, "API 密钥", &api_key);
        note(
            layout,
            mtm,
            "文本框按回车保存。密钥只保存在这台电脑上，不会随配置文件导出，也不显示已填的值。",
        );
        let test = button(mtm, "测试连接", Setting::TestCloud, target);
        layout.place(&test, PAGE_PADDING, 120.0, ROW_HEIGHT + 4.0);
        layout.next_row(ROW_HEIGHT + 4.0);
        note(
            layout,
            mtm,
            "用上面填的地址、模型、密钥发一条最小请求，结果显示在窗口底部。输入法进程看不到终端里的代理变量，走不通时先查这个。",
        );
        Self {
            local_model,
            enabled,
            slots,
            chinese_words,
            english_words,
            gloss,
            translation,
            gloss_channel,
            translation_channel,
            sentence,
            english_sentence,
            scenarios_enabled,
            scenarios,
            base_url,
            model,
            api_key,
            test,
        }
    }

    /// `key_present` 是密钥已经有了（环境或配置里）；密钥框永远不回显值，只换占位文字。
    /// `model_present` 是包里或用户目录里有模型文件，没有就把本地模型的勾选灰掉；云联想关着时它下面的项全灰。
    pub fn sync(&self, config: &Config, key_present: bool, model_present: bool) {
        set_checked(&self.local_model, config.model.enabled && model_present);
        self.local_model.setEnabled(model_present);
        set_checked(&self.enabled, config.predict.enabled);
        let cloud = config.predict.enabled;
        self.slots.setEnabled(cloud);
        set_checked(&self.chinese_words, config.predict.chinese_words);
        self.chinese_words.setEnabled(cloud);
        set_checked(&self.english_words, config.predict.english_words);
        self.english_words.setEnabled(cloud);
        set_checked(&self.gloss, config.predict.gloss.enabled);
        self.gloss.setEnabled(cloud);
        set_checked(&self.translation, config.predict.translation.enabled);
        self.translation.setEnabled(cloud);
        select(
            &self.gloss_channel,
            qingjian_predict::TranslationChannel::ALL
                .iter()
                .position(|c| *c == config.predict.gloss.channel),
        );
        self.gloss_channel
            .setEnabled(cloud && config.predict.gloss.enabled);
        select(
            &self.translation_channel,
            qingjian_predict::TranslationChannel::ALL
                .iter()
                .position(|c| *c == config.predict.translation.channel),
        );
        self.translation_channel
            .setEnabled(cloud && config.predict.translation.enabled);

        set_checked(&self.sentence, config.predict.sentence);
        set_checked(&self.english_sentence, config.predict.english_sentence);
        self.sentence.setEnabled(cloud);
        self.english_sentence.setEnabled(cloud);
        set_checked(&self.scenarios_enabled, config.predict.scenarios_enabled);
        self.scenarios_enabled.setEnabled(cloud);
        self.scenarios
            .setEnabled(cloud && config.predict.scenarios_enabled);
        self.scenarios
            .setStringValue(&NSString::from_str(&config.predict.scenarios));
        self.base_url.setEnabled(cloud);
        self.model.setEnabled(cloud);
        self.api_key.setEnabled(cloud);
        self.test.setEnabled(cloud);
        select(&self.slots, Some(config.predict.slots.min(MAX_CLOUD_SLOTS)));
        self.base_url
            .setStringValue(&NSString::from_str(&config.predict.base_url));
        self.model
            .setStringValue(&NSString::from_str(&config.predict.model));
        self.api_key.setStringValue(&NSString::from_str(""));
        let hint = if key_present {
            "已设置，输入新值可替换"
        } else {
            "未设置"
        };
        self.api_key
            .setPlaceholderString(Some(&NSString::from_str(hint)));
    }
}
