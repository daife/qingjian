//! 「云服务」页：本地整句模型开关（`[model]`）、`[predict]` 各项与「测试连接」（后台线程跑）。

use qingjian_predict::{ConnectionTest, PredictConfig};
use windows_reactor::*;

use crate::panel::cloud_status::CloudStatus;
use crate::panel::controls::{field, labeled, note, page};
use crate::panel::{Message, Settings};

/// 后台跑一次连通性测试，轮询到有结果或被取消。
pub(crate) fn run_test(
    config: &PredictConfig,
    cancel: &CancellationToken,
) -> Result<String, String> {
    let test = ConnectionTest::start(config).map_err(|error| error.to_string())?;
    loop {
        if cancel.is_cancelled() {
            return Err("已取消".to_owned());
        }
        if let Some(result) = test.poll() {
            return result
                .map(|report| {
                    format!(
                        "连接成功：模型 {}，耗时 {} ms",
                        report.model,
                        report.elapsed.as_millis()
                    )
                })
                .map_err(|error| error.to_string());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

pub(crate) fn view(settings: &Settings, context: &mut ViewContext<Settings>) -> View {
    let p = &settings.config.predict;
    let status = match &settings.cloud_status {
        CloudStatus::Idle => String::new(),
        CloudStatus::Testing => "测试中…".to_owned(),
        CloudStatus::Ok(message) => message.clone(),
        CloudStatus::Failed(message) => format!("失败：{message}"),
    };
    let rows = [
        field(
            "本地整句模型",
            "随包的小模型在本机给整句候选重新排序，全程离线；停键后几十毫秒生效。关掉只用词库统计。",
            ToggleSwitch::new()
                .is_on(settings.config.model.enabled)
                .on_toggled(context.callback(Message::LocalModel)),
        ),
        field(
            "启用云服务",
            "开启后组句时会把光标附近的几十个字发给下面的服务，按下面的开关提供候选词、整句补全和翻译；密码框不发送。",
            ToggleSwitch::new()
                .is_on(p.enabled)
                .on_toggled(context.callback(Message::CloudEnabled)),
        ),
        field(
            "云端词格数",
            "云端词到了只补进第一页末尾这几格，前面的本地候选不动；没到就什么都不变。0 = 不请求云端候选词。",
            NumberBox::new()
                .minimum(0.0)
                .maximum(9.0)
                .value(p.slots as f64)
                .on_value_changed(context.callback(Message::CloudSlots)),
        ),
        field(
            "中文云端候选词联想",
            "停键后补充当前输入的词候选，数字键选择。",
            ToggleSwitch::new()
                .is_enabled(p.enabled)
                .is_on(p.chinese_words)
                .on_toggled(context.callback(Message::CloudChineseWords)),
        ),
        field(
            "英文云端候选词联想",
            "停键后补充当前输入的词候选，数字键选择。",
            ToggleSwitch::new()
                .is_enabled(p.enabled)
                .is_on(p.english_words)
                .on_toggled(context.callback(Message::CloudEnglishWords)),
        ),
        field(
            "中文整句补全",
            "中文输入时给出整句补全，按 Tab 采用。",
            ToggleSwitch::new()
                .is_enabled(p.enabled)
                .is_on(p.sentence)
                .on_toggled(context.callback(Message::CloudSentence)),
        ),
        field(
            "英文整句补全",
            "英文输入时给出整句补全，按 Tab 采用。",
            ToggleSwitch::new()
                .is_enabled(p.enabled)
                .is_on(p.english_sentence)
                .on_toggled(context.callback(Message::CloudEnglishSentence)),
        ),
        field(
            "缺失释义补全",
            "使用所选渠道提供翻译。",
            ToggleSwitch::new()
                .is_enabled(p.enabled)
                .is_on(p.gloss.enabled)
                .on_toggled(context.callback(Message::CloudGloss)),
        ),
        field(
            "释义补全渠道",
            "共用大模型云服务使用下方的地址、模型与密钥。",
            ComboBox::new()
                .is_enabled(p.enabled && p.gloss.enabled)
                .items_source(
                    qingjian_predict::TranslationChannel::ALL
                        .iter()
                        .map(|channel| channel.label().to_owned())
                        .collect::<Vec<_>>(),
                )
                .selected_index(
                    qingjian_predict::TranslationChannel::ALL
                        .iter()
                        .position(|channel| *channel == p.gloss.channel),
                )
                .on_selection_changed(context.callback(Message::CloudGlossChannel)),
        ),
        field(
            "翻译选中文字",
            "使用所选渠道提供翻译。",
            ToggleSwitch::new()
                .is_enabled(p.enabled)
                .is_on(p.translation.enabled)
                .on_toggled(context.callback(Message::CloudTranslation)),
        ),
        field(
            "选区翻译渠道",
            "共用大模型云服务使用下方的地址、模型与密钥。",
            ComboBox::new()
                .is_enabled(p.enabled && p.translation.enabled)
                .items_source(
                    qingjian_predict::TranslationChannel::ALL
                        .iter()
                        .map(|channel| channel.label().to_owned())
                        .collect::<Vec<_>>(),
                )
                .selected_index(
                    qingjian_predict::TranslationChannel::ALL
                        .iter()
                        .position(|channel| *channel == p.translation.channel),
                )
                .on_selection_changed(context.callback(Message::CloudTranslationChannel)),
        ),
        field(
            "联想 & 整句补全场景",
            "开启后，将填写的常用场景发送给云服务，供中英文联想和整句补全参考。",
            ToggleSwitch::new()
                .is_enabled(p.enabled)
                .is_on(p.scenarios_enabled)
                .on_toggled(context.callback(Message::CloudScenariosEnabled)),
        ),
        field(
            "常用场景",
            "自行填写，用逗号或分号分隔，例如：日常聊天，我的世界游戏，IT 行业。",
            TextBox::new()
                .is_enabled(p.enabled && p.scenarios_enabled)
                .text(p.scenarios.clone())
                .on_text_changed(context.callback(Message::CloudScenarios)),
        ),
        field(
            "接口地址",
            "",
            TextBox::new()
                .text(p.base_url.clone())
                .on_text_changed(context.callback(Message::CloudBaseUrl)),
        ),
        field(
            "模型",
            "",
            TextBox::new()
                .text(p.model.clone())
                .on_text_changed(context.callback(Message::CloudModel)),
        ),
        field(
            "API 密钥",
            "只保存在这台电脑上，不会随配置文件导出，也不显示已填的值。留空则读环境变量 QINGJIAN_API_KEY。",
            PasswordBox::new()
                .password(p.api_key.clone().unwrap_or_default())
                .placeholder_text("留空则读环境变量 QINGJIAN_API_KEY")
                .on_password_changed(context.callback(Message::CloudApiKey)),
        ),
        labeled(
            "",
            StackPanel::new()
                .orientation(Orientation::Horizontal)
                .spacing(12.0)
                .children((
                    Button::new()
                        .on_click(context.message(Message::TestConnection))
                        .content("测试连接"),
                    TextBlock::new().text(status),
                )),
        ),
        note(
            "用上面填的地址、模型、密钥发一条最小请求。走不通时先查这里；Server 进程看不到终端里的代理变量。",
        ),
    ];
    page("云服务", StackPanel::new().spacing(16.0).children(rows))
}
