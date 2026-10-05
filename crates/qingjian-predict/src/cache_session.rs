//! 按用途固定云端缓存会话；不是用户身份，不含输入内容或个人信息。

use qingjian_core::PredictionKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CacheSession {
    Chinese,

    English,

    Gloss,

    Translation,

    Question,
}

impl CacheSession {
    pub const ALL: [Self; 5] = [
        Self::Chinese,
        Self::English,
        Self::Gloss,
        Self::Translation,
        Self::Question,
    ];

    pub fn for_prediction(kind: PredictionKind) -> Self {
        match kind {
            PredictionKind::English => Self::English,
            PredictionKind::Translate => Self::Translation,
            PredictionKind::Compose => Self::Chinese,
            PredictionKind::Question => Self::Question,
        }
    }

    /// 固定且带应用前缀，重启、切模式与重建客户端均不改变。
    pub fn id(self) -> &'static str {
        match self {
            Self::Chinese => "qingjian-chinese-prediction",
            Self::English => "qingjian-english-prediction",
            Self::Gloss => "qingjian-gloss-completion",
            Self::Translation => "qingjian-selection-translation",
            Self::Question => "qingjian-character-question",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CacheSession;
    use qingjian_core::PredictionKind;

    #[test]
    fn five_purposes_have_distinct_valid_stable_ids() {
        let ids: std::collections::HashSet<_> = CacheSession::ALL
            .map(CacheSession::id)
            .into_iter()
            .collect();
        assert_eq!(ids.len(), 5);
        for id in ids {
            assert!(id.len() <= 256);
            assert!(
                id.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            );
        }
        for (kind, session) in [
            (PredictionKind::Compose, CacheSession::Chinese),
            (PredictionKind::English, CacheSession::English),
            (PredictionKind::Translate, CacheSession::Translation),
            (PredictionKind::Question, CacheSession::Question),
        ] {
            assert_eq!(CacheSession::for_prediction(kind), session);
        }
    }
}
