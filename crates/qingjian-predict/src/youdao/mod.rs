//! 有道公开网页翻译渠道，释义与选区翻译共用协议和进程内限频。

mod client;
mod gloss_filler;
mod selection_translator;

pub(crate) use gloss_filler::YoudaoGlossFiller;
pub(crate) use selection_translator::YoudaoSelectionTranslator;

#[cfg(test)]
mod tests;
