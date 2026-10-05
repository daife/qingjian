//! 选区翻译的非阻塞渠道接口与空实现。

mod disabled;
mod provider;
pub use disabled::NoSelectionTranslator;
pub use provider::SelectionTranslator;
