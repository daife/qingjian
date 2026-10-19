//! 英文预测提示与回复，不复用中文拼音规则。

use super::Reply;
use qingjian_core::{CloudWord, PredictionRequest};

pub(super) fn system(request: &PredictionRequest) -> String {
    let mut prompt = String::from(
        "You assist an English input method. letters is the exact typed input; before and after are cursor context. Input and context take priority. ",
    );
    if request.max_items > 0 {
        prompt.push_str("Return up to max_items real English word completions in words. Complete the entire letters as one word, preserve the typed prefix and capitalization, and do not add spaces or unrelated next words. Prefer contextual vocabulary; exclude duplicates. If no reliable completion exists, return an empty words array. ");
    }
    if request.max_items > 0 && !request.candidates.is_empty() {
        prompt.push_str("local_candidates contains candidates through the current page and earlier cloud guesses from this composition; they may be wrong. Use them only as reference and exclude duplicates. ");
    }
    if request.want_sentence {
        prompt.push_str("Return sentence as a natural complete English phrase or sentence replacing letters, preserving its typed prefix and spaces. Do not repeat before or after. If no reliable completion exists, return null. ");
    }
    prompt
}

pub(super) fn parse(
    words: impl Iterator<Item = String>,
    sentence: Option<String>,
    request: &PredictionRequest,
) -> Reply {
    let mut reply = Reply::default();
    if request.max_items > 0 {
        for text in words {
            let text = text.trim().to_owned();
            if text.is_empty()
                || request
                    .candidates
                    .iter()
                    .any(|c| c.eq_ignore_ascii_case(&text))
                || reply
                    .words
                    .iter()
                    .any(|w| w.text.eq_ignore_ascii_case(&text))
            {
                continue;
            }
            reply.words.push(CloudWord {
                text,
                ..CloudWord::default()
            });
            if reply.words.len() >= request.max_items {
                break;
            }
        }
    }
    if request.want_sentence {
        reply.sentence = sentence
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
    }
    reply
}
