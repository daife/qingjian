//! 输入串与读音键的选择撤销必须在重启后仍然生效。

use qingjian_core::Learner;
use qingjian_learning::FrequencyLearner;

#[test]
fn undo_after_flush_persists_both_choice_keys() {
    let dir = std::env::temp_dir().join(format!(
        "qingjian-choice-undo-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("user.tsv");
    let mut learner = FrequencyLearner::from_path(&path).unwrap();
    learner.record_choice("wod", "wode", "我的");
    learner.record_choice("kaifa", "kaifa", "开发");
    learner.flush();
    assert!(!learner.has_unsaved());

    learner.unrecord_choice("wod", "wode", "我的");
    assert!(learner.has_unsaved());
    learner.flush();
    let restored = FrequencyLearner::from_path(&path).unwrap();
    assert_eq!(restored.choice_weight("wod", "我的"), 0);
    assert_eq!(restored.choice_weight("wode", "我的"), 0);
    assert_eq!(restored.choice_weight("kaifa", "开发"), 1);
    std::fs::remove_dir_all(dir).unwrap();
}
