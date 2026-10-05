//! 场景参考的作用范围与旧配置兼容。
use super::*;

#[test]
fn scenarios_only_guide_chinese_and_english_predictions() {
    let mut request = request("zhang'tao", true);
    for kind in [PredictionKind::Compose, PredictionKind::English] {
        request.kind = kind;
        let (system, user) =
            prediction_prompts(&request, " 日常聊天， Minecraft ;IT 行业\n日常聊天； ");
        let value: serde_json::Value = serde_json::from_str(&user).unwrap();
        assert_eq!(
            value["user_scenarios"],
            serde_json::json!(["日常聊天", "Minecraft", "IT 行业"])
        );
        assert!(system.contains("when compatible with the typed input and immediate context"));
        assert_eq!(
            prediction_prompts(&request, " ， ;\n"),
            (system_prompt(&request), user_prompt(&request))
        );
    }
    for kind in [PredictionKind::Question, PredictionKind::Translate] {
        request.kind = kind;
        assert_eq!(
            prediction_prompts(&request, "Minecraft"),
            (system_prompt(&request), user_prompt(&request))
        );
    }
}

#[test]
fn scenario_config_defaults_and_round_trips() {
    let config = crate::PredictConfig::default();
    assert!(!config.scenarios_enabled);
    assert!(config.scenarios.is_empty());
    let config = crate::PredictConfig {
        scenarios_enabled: true,
        scenarios: "日常聊天，Minecraft".into(),
        ..config
    };
    let encoded = serde_json::to_string(&config).unwrap();
    let decoded: crate::PredictConfig = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, config);
    let legacy: crate::PredictConfig = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
    assert!(!legacy.scenarios_enabled);
}
