//! 候选翻页、译词上屏与快捷键的回归。
use super::support::{compose, key, router};
use qingjian_platform::protocol::{ClientMessage, KeyEvent, KeyModifiers, KeyOutcome, SessionId};

#[test]
fn tab_and_backtab_page_boundaries_and_current_page_selection() {
    for size in [1, 4, 5, 9] {
        let mut router = router(size);
        let normal = KeyModifiers::default();
        let shift = KeyModifiers {
            shift: true,
            ..normal
        };
        assert_eq!(key(&mut router, 9, None, normal).0, KeyOutcome::Passthrough);
        assert_eq!(key(&mut router, 9, None, shift).0, KeyOutcome::Passthrough);
        compose(&mut router, "qq", normal);
        assert_eq!(key(&mut router, 9, None, shift).2.page, 0);
        let last = 9_usize.div_ceil(size) - 1;
        for page in 1..=last {
            let result = key(&mut router, 9, None, normal);
            assert_eq!(result.0, KeyOutcome::Consumed);
            assert_eq!((result.2.page, result.2.highlight), (page, 0));
        }
        let frame = key(&mut router, 9, None, normal).2;
        assert_eq!(frame.page, last);
        let expected = frame.candidates.items[0].text.clone();
        assert_eq!(
            key(&mut router, b'1' as u32, Some('1'), normal).1,
            Some(expected)
        );
        compose(&mut router, "qq", normal);
        key(&mut router, 0x22, None, normal);
        assert_eq!(key(&mut router, 9, None, shift).2.page, 0);
        assert_eq!(key(&mut router, 0x21, None, normal).2.page, 0);
    }
}
#[test]
fn shift_tab_precedes_prediction_and_english_commit() {
    let mut router = router(1);
    let normal = KeyModifiers::default();
    compose(&mut router, "qq", normal);
    router.sentence = Some("可控补全".into());
    let result = key(
        &mut router,
        9,
        None,
        KeyModifiers {
            shift: true,
            ..normal
        },
    );
    assert_eq!(result.1, None);
    assert_eq!(router.sentence.as_deref(), Some("可控补全"));
    assert_eq!(
        key(&mut router, 9, None, normal).1.as_deref(),
        Some("可控补全")
    );
    let english = KeyModifiers {
        english_mode: true,
        ..normal
    };
    compose(&mut router, "hel", english);
    key(&mut router, 0x22, None, english);
    let previous = key(
        &mut router,
        9,
        None,
        KeyModifiers {
            shift: true,
            ..english
        },
    );
    assert_eq!(previous.1, None);
    assert_eq!(previous.2.page, 0);
    let expected = previous.2.candidates.items[previous.2.highlight]
        .text
        .clone();
    assert_eq!(key(&mut router, 9, None, english).1, Some(expected));
}
#[test]
fn tab_with_raw_input_and_no_candidates_is_consumed_without_commit() {
    let mut router = router(5);
    compose(&mut router, "zzzz", KeyModifiers::default());
    let result = key(&mut router, 9, None, KeyModifiers::default());
    assert_eq!(result.0, KeyOutcome::Consumed);
    assert_eq!(result.1, None);
    assert_eq!(result.2.page, 0);
}

#[test]
fn disabled_digit_shortcuts_release_chords_and_keep_normal_selection() {
    let mut router = router(5);
    router.config.translation_keys = (KeyModifiers::default(), KeyModifiers::default());
    router.config.delete_keys = KeyModifiers::default();
    let normal = KeyModifiers::default();
    compose(&mut router, "qq", normal);
    let ctrl = KeyModifiers {
        ctrl: true,
        ..normal
    };
    let result = key(&mut router, b'1' as u32, Some('1'), ctrl);
    assert_eq!(result.0, KeyOutcome::Passthrough);
    assert_eq!(result.1, None);
    assert_eq!(router.engine.composition().text(), "qq");
    assert_eq!(
        key(&mut router, b'1' as u32, Some('1'), normal).1,
        Some("第1项".to_owned())
    );
}

#[test]
fn english_tab_accepts_sentence_before_local_word() {
    let mut router = router(5);
    let english = KeyModifiers {
        english_mode: true,
        ..Default::default()
    };
    compose(&mut router, "hello", english);
    router.sentence = Some("hello world".into());
    assert_eq!(
        key(&mut router, 9, None, english).1.as_deref(),
        Some("hello world")
    );
}

#[test]
fn translation_hold_with_no_candidates_submits_the_whole_buffer() {
    struct Service;
    impl qingjian_core::SelectionTranslator for Service {
        fn submit(&mut self, request: qingjian_core::PredictionRequest) {
            assert_eq!(request.text, "hello world");
        }
        fn poll(&mut self) -> Option<qingjian_core::Prediction> {
            None
        }
    }
    let mut router = router(5);
    key(&mut router, 0, None, KeyModifiers::default());
    router.engine.set_selection_translator(Box::new(Service));
    router.engine.set_translation_commit_fallback(true);
    router.engine.set_english_mode(true);
    router.engine.set_input("hello world");
    router.recompose();
    let result = router.handle(ClientMessage::Key {
        session: SessionId(1),
        event: KeyEvent {
            long_press: true,
            ..KeyEvent::new(13, None, KeyModifiers::default())
        },
    });
    assert!(result.is_some());
    assert!(router.engine.translation_commit_pending());
    assert_eq!(router.engine.composition().text(), "hello world");
}
/// 组句中会转全角的标点：先把高亮候选上屏再补标点（`ni,` 出「你，」）；
/// 半角标点模式下候选照样上屏、标点按半角补。`,` `.` 配成翻页键时翻页优先，不上屏。
#[test]
fn punctuation_commits_highlighted_candidate() {
    let mut router = router(5);
    router.config.punct_commits = true;
    let normal = KeyModifiers::default();
    compose(&mut router, "ni", normal);
    let result = key(&mut router, 0xBC, Some(','), normal);
    assert_eq!(result.0, KeyOutcome::Consumed);
    assert_eq!(result.1.as_deref(), Some("你，"));
    router.config.full_width = false;
    compose(&mut router, "ni", normal);
    let result = key(&mut router, 0xBC, Some(','), normal);
    assert_eq!(result.1.as_deref(), Some("你,"));
    router.config.page_keys = (',', '.');
    compose(&mut router, "ni", normal);
    let result = key(&mut router, 0xBC, Some(','), normal);
    assert_eq!(result.0, KeyOutcome::Consumed);
    assert_eq!(result.1, None);
    assert_eq!(result.2.page, 0);
}
/// 不会转全角的符号（`-`）仍进英文直输段；`'` 是隔音符，进缓冲区不触发上屏，随后的标点照常上屏候选。
#[test]
fn unconvertible_symbols_do_not_commit_candidates() {
    let mut router = router(5);
    router.config.punct_commits = true;
    let normal = KeyModifiers::default();
    compose(&mut router, "ni", normal);
    let result = key(&mut router, 0xBD, Some('-'), normal);
    assert_eq!(result.1, None);
    let result = key(&mut router, 0x20, Some(' '), normal);
    assert_eq!(result.1.as_deref(), Some("ni- "));
    compose(&mut router, "ni", normal);
    let result = key(&mut router, 0xDE, Some('\''), normal);
    assert_eq!(result.1, None);
    let result = key(&mut router, 0xBC, Some(','), normal);
    assert_eq!(result.1.as_deref(), Some("你，"));
}
/// 直通了数字再组句，`Punctuation` 的「数字后的点保持半角」状态要跟着刷新：`3` + `ni` + `.` 出「你。」不出「你.」。
#[test]
fn digit_then_composition_resets_decimal_point_state() {
    let mut router = router(5);
    router.config.punct_commits = true;
    let normal = KeyModifiers::default();
    key(&mut router, 0x33, Some('3'), normal);
    compose(&mut router, "ni", normal);
    let result = key(&mut router, 0xBE, Some('.'), normal);
    assert_eq!(result.1.as_deref(), Some("你。"));
}
/// 关掉 `[general] punct_commits`：组句中的标点回到老行为——进英文直输段，不再上屏候选；开回来恢复。
#[test]
fn punct_commits_off_keeps_punctuation_in_raw_segment() {
    let mut router = router(5);
    let normal = KeyModifiers::default();
    router.config.punct_commits = false;
    compose(&mut router, "ni", normal);
    assert_eq!(key(&mut router, 0xBC, Some(','), normal).1, None);
    assert_eq!(
        key(&mut router, 0x20, Some(' '), normal).1.as_deref(),
        Some("ni, ")
    );
    router.config.punct_commits = true;
    compose(&mut router, "ni", normal);
    assert_eq!(
        key(&mut router, 0xBC, Some(','), normal).1.as_deref(),
        Some("你，")
    );
}
