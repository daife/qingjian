//! 提示词与回复解析。模型只输出约定的 JSON，其余一概不信；拼音校验在 Core 里再做一遍。

use qingjian_core::{CloudWord, PredictionKind, PredictionRequest};
use serde::{Deserialize, Serialize};

/// 联想规则按输出任务组合，避免只补句时发送选词说明。
const COMPOSE_CONTEXT: &str = "拼音输入法联想。输入 JSON：letters 为原始字母，pinyin 为可能错误的音节切分（' 分隔，单字母为声母缩写），syllables 为参考音节数；before/after 为光标上下文，缺省为空。允许纠正错字、漏字、多字；优先按 letters 与上下文判断。local_sentence/local_candidates 是可能错误的本地猜测，不要照抄或被其带偏。只输出 JSON。";
const WORD_RULES: &str = "words 返回 0 至 max_items 个真实词语，按可能性排序，优先补充术语、新词、专名及正确同音词；与本地候选重复或没有更好答案时返回 []。单字母是声母缩写，不要硬凑或生造词。每项 {\"text\":\"词\",\"pinyin\":\"正确全拼\"}，拼音空格分隔、字数等于音节数，不增减用户意图的音节。不确定少给。";
const SENTENCE_RULES: &str = "sentence 返回以所输入词语开头的自然完整短句，可补全未敲完的拼音；只替换当前输入，不复述 before/after，语言随上下文，无上下文用最常见表达。";

/// 问字模式：回答问题，不返回问题的汉字写法。
pub const QUESTION_SYSTEM_PROMPT: &str = "拼音问字助手。输入 JSON：letters 为可能有错漏的拼音字母，pinyin 为可能错误的切分（' 分隔），question 为不可靠的本地汉字还原，max_items 为答案上限。先还原问题，再给可直接上屏的字、词或短事实答案；字形、部件、读音、含义描述均可，如 san ge mu shi shen me zi 答 森。不要把问题本身当答案。只输出 JSON：{\"answers\":[{\"text\":\"答案\",\"pinyin\":\"sēn\"}]}，按可能性排序，不超过 max_items；中文用带声调拼音，非中文读音为空；不解释，不确定少给或 []。";

/// 翻译保留完整原文及方向兜底。
pub const TRANSLATE_SYSTEM_PROMPT: &str = "把 JSON 的 text 完整自然地译成 target_language（zh 中文、en 英语、ja 日语），保留语气、换行与标点。原文已是目标语言时，非中文改译中文，中文原样返回。只输出 JSON：{\"sentence\":\"译文\"}，无解释或前缀。";

pub fn system_prompt(request: &PredictionRequest) -> String {
    match request.kind {
        PredictionKind::English => {
            "Complete the English text in letters naturally using before and after as context. Return the full replacement for letters, preserving the typed prefix and its spaces. Do not repeat before or after. Return only JSON: {\"sentence\": \"completed English text\"}."
        }
        PredictionKind::Compose => {
            let mut prompt = String::from(COMPOSE_CONTEXT);
            if request.max_items > 0 { prompt.push_str(WORD_RULES); }
            if request.want_sentence { prompt.push_str(SENTENCE_RULES); }
            prompt.push_str("输出对象含 words 和 sentence；未请求词时 words=[]，未请求整句时 sentence=null。");
            return prompt;
        },
        PredictionKind::Question => QUESTION_SYSTEM_PROMPT,
        PredictionKind::Translate => TRANSLATE_SYSTEM_PROMPT,
    }.to_owned()
}

/// 发给模型的用户消息：仅序列化任务所需字段，省略空上下文和本地提示。
#[derive(Serialize)]
struct UserMessage<'a> {
    letters: &'a str,

    #[serde(skip_serializing_if = "str::is_empty")]
    pinyin: &'a str,

    syllables: usize,

    #[serde(skip_serializing_if = "str::is_empty")]
    before: &'a str,

    #[serde(skip_serializing_if = "str::is_empty")]
    after: &'a str,

    #[serde(skip_serializing_if = "str::is_empty")]
    local_sentence: &'a str,

    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    local_candidates: &'a [String],

    max_items: usize,

    want_sentence: bool,
}

/// 英文补全只需输入和上下文。
#[derive(Serialize)]
struct EnglishMessage<'a> {
    letters: &'a str,

    #[serde(skip_serializing_if = "str::is_empty")]
    before: &'a str,

    #[serde(skip_serializing_if = "str::is_empty")]
    after: &'a str,
}

/// 问字模式发给模型的用户消息。
#[derive(Serialize)]
struct QuestionMessage<'a> {
    letters: &'a str,

    #[serde(skip_serializing_if = "str::is_empty")]
    pinyin: &'a str,

    #[serde(skip_serializing_if = "str::is_empty")]
    question: &'a str,

    max_items: usize,
}

/// 翻译发给模型的用户消息。
#[derive(Serialize)]
struct TranslateMessage<'a> {
    text: &'a str,

    target_language: &'a str,
}

pub fn user_prompt(request: &PredictionRequest) -> String {
    if request.kind == PredictionKind::English {
        return serde_json::to_string(&EnglishMessage {
            letters: &request.letters,
            before: &request.before,
            after: &request.after,
        })
        .unwrap_or_default();
    }
    if request.kind == PredictionKind::Translate {
        return serde_json::to_string(&TranslateMessage {
            text: &request.text,
            target_language: &request.target_language,
        })
        .unwrap_or_default();
    }
    if request.kind == PredictionKind::Question {
        return serde_json::to_string(&QuestionMessage {
            letters: &request.letters,
            pinyin: &request.pinyin,
            question: &request.guess,
            max_items: request.max_items,
        })
        .unwrap_or_default();
    }
    serde_json::to_string(&UserMessage {
        letters: &request.letters,
        pinyin: &request.pinyin,
        syllables: request.syllables,
        before: &request.before,
        after: &request.after,
        local_sentence: &request.guess,
        local_candidates: &request.candidates,
        max_items: request.max_items,
        want_sentence: request.want_sentence,
    })
    .unwrap_or_default()
}

/// 解析后的回复。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reply {
    /// 云端词（组句联想）或答案（问字模式，音节为空、带读音）。
    pub words: Vec<CloudWord>,

    /// 整句补全。
    pub sentence: Option<String>,
}

impl Reply {
    /// 什么都没给：没有词也没有整句。
    pub fn is_empty(&self) -> bool {
        self.words.is_empty() && self.sentence.is_none()
    }
}

/// 模型回复的原始形状，缺的字段当空。
#[derive(Deserialize, Default)]
#[serde(default)]
struct RawReply {
    words: Vec<RawWord>,

    sentence: Option<String>,

    /// 问字模式的答案。
    answers: Vec<RawWord>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawWord {
    text: String,

    pinyin: String,
}

/// 解析模型回复：去空、去重、去换行，截到 `max_items`。不要与本地首选相同的词，也不要没给拼音的词。
pub fn parse_reply(content: &str, request: &PredictionRequest) -> Reply {
    let raw: RawReply = match serde_json::from_str(content.trim()) {
        Ok(raw) => raw,
        Err(_) => return Reply::default(),
    };
    if request.kind == PredictionKind::Question {
        return parse_answers(raw.answers, request.max_items);
    }
    if matches!(
        request.kind,
        PredictionKind::Translate | PredictionKind::English
    ) {
        // 译文保留换行（原文可能是多段），只去首尾空白
        return Reply {
            words: Vec::new(),
            sentence: raw
                .sentence
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty()),
        };
    }
    let mut reply = Reply::default();
    let mut seen: Vec<String> = Vec::new();
    let first_local = request.candidates.first().map(String::as_str);
    for word in raw.words {
        let text = clean(&word.text);
        let syllables: Vec<String> = word
            .pinyin
            .split(|c: char| c.is_whitespace() || c == '\'')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_ascii_lowercase())
            .collect();
        if text.is_empty()
            || syllables.is_empty()
            || Some(text.as_str()) == first_local
            || seen.contains(&text)
        {
            continue;
        }
        seen.push(text.clone());
        reply.words.push(CloudWord {
            text,
            syllables,
            reading: None,
        });
        if reply.words.len() >= request.max_items {
            break;
        }
    }
    if request.want_sentence {
        reply.sentence = raw
            .sentence
            .map(|s| strip_before(&clean(&s), &request.before))
            .filter(|s| !s.is_empty() && Some(s.as_str()) != first_local);
    }
    reply
}

/// 问字模式的答案：去空、去重，拼音只是显示用的读音，不校验。
fn parse_answers(answers: Vec<RawWord>, max_items: usize) -> Reply {
    let mut reply = Reply::default();
    for answer in answers {
        let text = clean(&answer.text);
        if text.is_empty() || reply.words.iter().any(|w| w.text == text) {
            continue;
        }
        let reading = clean(&answer.pinyin);
        reply.words.push(CloudWord {
            text,
            syllables: Vec::new(),
            reading: (!reading.is_empty()).then_some(reading),
        });
        if reply.words.len() >= max_items {
            break;
        }
    }
    reply
}

/// 模型爱把 before 也抄进整句里；整句只替换拼音，所以把与 before 尾部重叠的开头去掉。
fn strip_before(sentence: &str, before: &str) -> String {
    let before: Vec<char> = before.chars().collect();
    let chars: Vec<char> = sentence.chars().collect();
    // 从最长的重叠开始试：before 的后 k 个字符 == sentence 的前 k 个字符
    for k in (1..=before.len().min(chars.len())).rev() {
        if before[before.len() - k..] == chars[..k] {
            return chars[k..]
                .iter()
                .collect::<String>()
                .trim_start()
                .to_owned();
        }
    }
    sentence.to_owned()
}

fn clean(text: &str) -> String {
    text.trim()
        .chars()
        .filter(|c| *c != '\n' && *c != '\r')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(pinyin: &str, want_sentence: bool) -> PredictionRequest {
        PredictionRequest {
            sequence: 1,
            kind: PredictionKind::Compose,
            before: "我们今天".into(),
            after: String::new(),
            pinyin: pinyin.into(),
            letters: pinyin.replace('\'', ""),
            syllables: 2,
            candidates: vec!["张涛".into(), "张贴".into()],
            guess: String::new(),
            max_items: 2,
            want_sentence,
            text: String::new(),
            target_language: String::new(),
        }
    }

    #[test]
    fn english_completion_preserves_spaces_and_needs_no_pinyin() {
        let mut request = request("", true);
        request.kind = PredictionKind::English;
        request.letters = "hello ".into();
        let prompt: serde_json::Value = serde_json::from_str(&user_prompt(&request)).unwrap();
        assert_eq!(prompt["letters"], "hello ");
        assert_eq!(prompt.as_object().unwrap().len(), 2);
        assert_eq!(prompt["before"], "我们今天");
        let reply = parse_reply(
            r#"{"sentence":"hello world","words":[{"text":"wrong","pinyin":""}]}"#,
            &request,
        );
        assert_eq!(reply.sentence.as_deref(), Some("hello world"));
        assert!(reply.words.is_empty());
        assert!(system_prompt(&request).contains("English"));
    }

    #[test]
    fn user_prompt_is_the_request_as_json() {
        let prompt = user_prompt(&request("zhang'tao", true));
        assert!(prompt.contains("\"letters\":\"zhangtao\""));
        assert!(prompt.contains("\"pinyin\":\"zhang'tao\""));
        assert!(prompt.contains("\"syllables\":2"));
        assert!(prompt.contains("\"local_candidates\":[\"张涛\",\"张贴\"]"));
    }

    #[test]
    fn compose_omits_empty_hints_and_selects_task_rules() {
        let mut request = request("zhang'tao", false);
        let user: serde_json::Value = serde_json::from_str(&user_prompt(&request)).unwrap();
        assert!(user.get("after").is_none());
        assert!(user.get("local_sentence").is_none());
        assert!(system_prompt(&request).contains(WORD_RULES));
        assert!(!system_prompt(&request).contains(SENTENCE_RULES));
        request.max_items = 0;
        request.want_sentence = true;
        assert!(!system_prompt(&request).contains(WORD_RULES));
        assert!(system_prompt(&request).contains(SENTENCE_RULES));
        request.candidates.clear();
        let user: serde_json::Value = serde_json::from_str(&user_prompt(&request)).unwrap();
        assert!(user.get("local_candidates").is_none());
    }

    #[test]
    fn reply_keeps_words_with_pinyin_and_drops_the_local_first() {
        let reply = r#"{"words": [{"text": "账套", "pinyin": "zhang tao"}, {"text": "张涛", "pinyin": "zhang tao"},
            {"text": "涨停", "pinyin": ""}, {"text": " 章台 ", "pinyin": "Zhang'Tai"}, {"text": "张套", "pinyin": "zhang tao"}],
            "sentence": " 账套已经建好了\n"}"#;
        let parsed = parse_reply(reply, &request("zhang'tao", true));
        let texts: Vec<(&str, Vec<&str>)> = parsed
            .words
            .iter()
            .map(|w| {
                (
                    w.text.as_str(),
                    w.syllables.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        assert_eq!(
            texts,
            [
                ("账套", vec!["zhang", "tao"]),
                ("章台", vec!["zhang", "tai"])
            ]
        );
        assert_eq!(parsed.sentence.as_deref(), Some("账套已经建好了"));
        // 没要整句就不收
        assert_eq!(
            parse_reply(reply, &request("zhang'tao", false)).sentence,
            None
        );
        // 整句里抄了 before 的，去掉重叠部分
        let echoed = r#"{"words": [], "sentence": "我们今天账套已经建好了"}"#;
        assert_eq!(
            parse_reply(echoed, &request("zhang'tao", true))
                .sentence
                .as_deref(),
            Some("账套已经建好了")
        );
        let partial = r#"{"words": [], "sentence": "今天账套已经建好了"}"#;
        assert_eq!(
            parse_reply(partial, &request("zhang'tao", true))
                .sentence
                .as_deref(),
            Some("账套已经建好了")
        );
    }

    #[test]
    fn question_replies_keep_answers_with_readings() {
        let mut question = request("san'ge'mu", false);
        question.kind = PredictionKind::Question;
        question.candidates.clear();
        assert!(user_prompt(&question).contains("\"letters\":\"sangemu\""));
        assert!(!user_prompt(&question).contains("before"));
        let reply = r#"{"answers": [{"text": "森", "pinyin": "sēn"}, {"text": "森", "pinyin": "sēn"}, {"text": "巴黎", "pinyin": ""}]}"#;
        let parsed = parse_reply(reply, &question);
        assert_eq!(parsed.words.len(), 2);
        assert_eq!(parsed.words[0].text, "森");
        assert_eq!(parsed.words[0].reading.as_deref(), Some("sēn"));
        assert!(parsed.words[0].syllables.is_empty());
        assert_eq!(parsed.words[1].reading, None);
        assert_eq!(parsed.sentence, None);
    }

    #[test]
    fn garbage_replies_are_empty() {
        assert_eq!(
            parse_reply("not json", &request("k", false)),
            Reply::default()
        );
        assert_eq!(
            parse_reply(r#"{"foo": 1}"#, &request("zt", false)),
            Reply::default()
        );
    }

    #[test]
    fn translate_requests_use_their_own_prompt_and_keep_the_translation() {
        let mut translate = request("", false);
        translate.kind = PredictionKind::Translate;
        translate.text = "我想去吃饭".to_owned();
        translate.target_language = "en".to_owned();
        assert_eq!(system_prompt(&translate), TRANSLATE_SYSTEM_PROMPT);
        assert!(user_prompt(&translate).contains("\"target_language\":\"en\""));
        let reply = parse_reply(r#"{"sentence": "  I want to go eat.\n"}"#, &translate);
        assert_eq!(reply.sentence.as_deref(), Some("I want to go eat."));
        assert!(reply.words.is_empty());
        assert!(
            parse_reply(r#"{"sentence": ""}"#, &translate)
                .sentence
                .is_none()
        );
    }
}
