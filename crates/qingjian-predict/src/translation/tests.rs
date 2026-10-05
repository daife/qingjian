//! 两种翻译分别控制，关闭时不要求密钥或启动联网通道。

use crate::{PredictConfig, TranslationChannel, build_gloss_filler, build_selection_translator};

#[test]
fn disabled_translation_tasks_do_not_require_credentials() {
    let mut config = PredictConfig {
        enabled: true,
        api_key_env: "QINGJIAN_TEST_MISSING_TRANSLATION_KEY".into(),
        ..PredictConfig::default()
    };
    config.gloss.enabled = false;
    config.translation.enabled = false;
    assert!(!build_gloss_filler(&config).unwrap().is_enabled());
    assert!(!build_selection_translator(&config).unwrap().is_enabled());
    config.gloss.enabled = true;
    assert!(build_gloss_filler(&config).is_err());
    assert!(!build_selection_translator(&config).unwrap().is_enabled());
    config.gloss.enabled = false;
    config.translation.enabled = true;
    assert!(!build_gloss_filler(&config).unwrap().is_enabled());
    assert!(build_selection_translator(&config).is_err());
    config.gloss.enabled = true;
    config.enabled = false;
    assert!(!build_gloss_filler(&config).unwrap().is_enabled());
    assert!(!build_selection_translator(&config).unwrap().is_enabled());
}

#[test]
fn legacy_config_preserves_behavior_and_translation_tasks_round_trip() {
    let legacy: PredictConfig = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
    assert!(legacy.chinese_words);
    assert!(!legacy.english_words);
    assert!(legacy.gloss.enabled && legacy.translation.enabled);
    assert_eq!(legacy.gloss.channel, TranslationChannel::Llm);
    let mut config = legacy;
    config.gloss.enabled = false;
    config.english_words = true;
    let decoded: PredictConfig =
        serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
    assert_eq!(decoded, config);
    assert!(
        serde_json::from_str::<PredictConfig>(r#"{"translation":{"channel":"unknown"}}"#).is_err()
    );
}

#[test]
fn youdao_channels_are_independent_and_need_no_llm_credentials() {
    let mut config = PredictConfig {
        enabled: true,
        api_key: None,
        api_key_env: "QINGJIAN_TEST_MISSING_TRANSLATION_KEY".into(),
        ..Default::default()
    };
    config.translation.channel = TranslationChannel::YoudaoPublic;
    assert!(build_selection_translator(&config).unwrap().is_enabled());
    assert!(build_gloss_filler(&config).is_err());
    config.gloss.channel = TranslationChannel::YoudaoPublic;
    config.translation.channel = TranslationChannel::Llm;
    assert!(build_gloss_filler(&config).unwrap().is_enabled());
    assert!(build_selection_translator(&config).is_err());
    config.translation.channel = TranslationChannel::YoudaoPublic;
    let encoded = serde_json::to_string(&config).unwrap();
    assert!(encoded.contains("youdao_public"));
    assert_eq!(
        serde_json::from_str::<PredictConfig>(&encoded).unwrap(),
        config
    );
    config.enabled = false;
    assert!(!build_gloss_filler(&config).unwrap().is_enabled());
    assert!(!build_selection_translator(&config).unwrap().is_enabled());
}
