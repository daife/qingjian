//! 英文句组设置的独立开关与上屏边界。

use super::support::{compose, key, router};
use qingjian_platform::protocol::{KeyModifiers, KeyOutcome};

#[test]
fn english_sentence_collects_space_punctuation_and_newline_then_tab_commits() {
    let mut router = router(5);
    router.engine.set_english_composition(true, true, true);
    let english = KeyModifiers {
        english_mode: true,
        ..Default::default()
    };
    compose(&mut router, "hello world,", english);
    assert_eq!(router.engine.composition().text(), "hello world,");
    let result = key(&mut router, 13, None, english);
    assert_eq!(result.0, KeyOutcome::Consumed);
    assert!(result.1.is_none());
    assert_eq!(router.engine.composition().text(), "hello world,\n");
    compose(&mut router, "how are you?", english);
    assert_eq!(
        key(&mut router, 9, None, english).1.as_deref(),
        Some("hello world,\nhow are you?")
    );
    assert!(router.engine.composition().is_empty());
}

#[test]
fn english_composition_switches_are_independent() {
    let english = KeyModifiers {
        english_mode: true,
        ..Default::default()
    };
    for (space, enter, punctuation) in [
        (true, false, false),
        (false, true, false),
        (false, false, true),
    ] {
        for (code, c, enabled) in [
            (32, Some(' '), space),
            (13, None, enter),
            (188, Some(','), punctuation),
        ] {
            let mut router = router(5);
            router
                .engine
                .set_english_composition(space, enter, punctuation);
            compose(&mut router, "hello", english);
            let result = key(&mut router, code, c, english);
            if enabled {
                assert!(result.1.is_none());
                assert_eq!(
                    router.engine.composition().text(),
                    format!("hello{}", c.unwrap_or('\n'))
                );
            } else {
                assert!(result.1.is_some());
                assert!(router.engine.composition().is_empty());
            }
        }
    }
}

#[test]
fn punctuation_composition_precedes_page_keys_and_chinese_space_still_commits() {
    let mut router = router(1);
    router.engine.set_english_composition(true, true, true);
    let english = KeyModifiers {
        english_mode: true,
        ..Default::default()
    };
    compose(&mut router, "hel", english);
    key(&mut router, 221, Some(']'), english);
    assert_eq!(router.engine.composition().text(), "hel]");
    key(&mut router, 27, None, english);
    compose(&mut router, "ni", KeyModifiers::default());
    assert_eq!(
        key(&mut router, 32, Some(' '), KeyModifiers::default())
            .1
            .as_deref(),
        Some("你")
    );
}

#[test]
fn english_without_candidates_keeps_passthrough_behavior() {
    let mut router = router(5);
    router.config.english_candidates = false;
    router.engine.set_english_composition(true, true, true);
    let english = KeyModifiers {
        english_mode: true,
        ..Default::default()
    };
    compose(&mut router, "hello ,", english);
    assert!(router.engine.composition().is_empty());
    assert_eq!(
        key(&mut router, 13, None, english).0,
        KeyOutcome::Passthrough
    );
}
