//! 注意力模式只改变绘制结果，保留候选与释义的上屏顺序。

use qingjian_core::{Candidate, CandidateKind, Language, Sense, Translation};
use qingjian_render::Tone;

use super::{for_display, from_candidate};

fn candidate(text: &str) -> Candidate {
    Candidate {
        text: text.into(),
        kind: CandidateKind::Cloud,
        syllables: Vec::new(),
        reading: None,
        translation: Some(Translation::new(
            Language::English,
            ["hello", "hi"]
                .into_iter()
                .map(|text| Sense {
                    text: text.into(),
                    part_of_speech: None,
                    reading: None,
                    fresh: false,
                })
                .collect(),
        )),
        aux_code: Some("abc".into()),
    }
}

#[test]
fn swap_only_changes_the_display_text_and_annotation() {
    let candidate = candidate("你好");
    let original = candidate.clone();
    let normal = from_candidate(2, &candidate, true);
    let swapped = for_display(2, &candidate, true, true);
    assert_eq!(swapped.text, "hello · hi");
    assert_eq!(swapped.annotation, vec![("你好".into(), Tone::Gloss)]);
    assert_eq!(swapped.index, normal.index);
    assert_eq!(swapped.code, normal.code);
    assert_eq!(swapped.cloud, normal.cloud);
    assert_eq!(candidate, original);
    assert_eq!(for_display(2, &candidate, true, false), normal);
}

#[test]
fn english_and_missing_or_blank_translations_keep_their_positions() {
    for text in ["hello", "你好"] {
        let mut candidate = candidate(text);
        if text == "hello" {
            assert_eq!(
                for_display(0, &candidate, false, true),
                from_candidate(0, &candidate, false)
            );
        }
        candidate.translation = None;
        assert_eq!(
            for_display(0, &candidate, false, true),
            from_candidate(0, &candidate, false)
        );
        candidate.translation = Some(Translation::new(
            Language::English,
            vec![Sense {
                text: " ".into(),
                part_of_speech: None,
                reading: None,
                fresh: false,
            }],
        ));
        assert_eq!(
            for_display(0, &candidate, false, true),
            from_candidate(0, &candidate, false)
        );
    }
}
