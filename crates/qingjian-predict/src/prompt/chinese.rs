//! 中文预测提示，按任务组合。

/// 联想规则按输出任务组合，避免只补句时发送选词说明。
pub(super) const COMPOSE_CONTEXT: &str = "拼音输入法联想。输入 JSON：letters 为原始字母，pinyin 为可能错误的音节切分（' 分隔，单字母为声母缩写），syllables 为参考音节数；before/after 为光标上下文，缺省为空。允许纠正错字、漏字、多字；优先按 letters 与上下文判断。";
pub(super) const WORD_RULES: &str = "words 返回 0 至 max_items 个真实词语，按可能性排序，优先补充术语、新词、专名及正确同音词；没有可靠候选时返回 []。单字母是声母缩写，不要硬凑或生造词。pinyin 给候选词的正确全拼，空格分隔、字数等于音节数；syllables 仅是输入切分的参考，纠错后以实际词语读音为准，不扩写成额外词语。不确定少给。";
pub(super) const SENTENCE_RULES: &str = "sentence 返回以所输入词语开头的自然完整短句，可补全未敲完的拼音；只替换当前输入，不复述 before/after，始终用中文，无上下文用最常见表达。没有可靠补全时返回 null。";

pub(super) fn system(request: &qingjian_core::PredictionRequest) -> String {
    let mut prompt = String::from(COMPOSE_CONTEXT);
    if request.max_items > 0 {
        if !request.candidates.is_empty() {
            prompt.push_str("local_candidates 包含前面页及当前页的候选，以及本轮输入中云端已经猜过的词，均可能有误，仅作参考，不要受其误导；words 不要与它重复。");
        }
        prompt.push_str(WORD_RULES);
    }
    if request.want_sentence {
        if !request.guess.is_empty() {
            prompt.push_str("local_sentence 是可能错误的本地整句猜测，仅作参考，不要受其误导。");
        }
        prompt.push_str(SENTENCE_RULES);
    }
    prompt
}
