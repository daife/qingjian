//! 按语言与启用任务生成最小回复结构。

use qingjian_core::{PredictionKind, PredictionRequest};
use serde_json::{Value, json};

pub(super) fn response_schema(request: &PredictionRequest) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    if request.max_items > 0 {
        let word = if request.kind == PredictionKind::English {
            json!({"type":"object", "properties":{"text":{"type":"string"}}, "required":["text"], "additionalProperties":false})
        } else {
            json!({"type":"object", "properties":{"text":{"type":"string"},"pinyin":{"type":"string"}}, "required":["text","pinyin"], "additionalProperties":false})
        };
        properties.insert(
            "words".into(),
            json!({"type":"array", "items":word, "maxItems":request.max_items}),
        );
        required.push("words");
    }
    if request.want_sentence {
        properties.insert("sentence".into(), json!({"type":["string","null"]}));
        required.push("sentence");
    }
    json!({"type":"object", "properties":properties, "required":required, "additionalProperties":false})
}
