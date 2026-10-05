//! 翻译任务的渠道配置，与联想策略分开。

mod channel;
mod config;
pub use channel::TranslationChannel;
pub use config::TranslationConfig;

mod cloud_translator;

/// 选区翻译的渠道装配入口，不要求翻译服务实现联想。
pub fn build_selection_translator(
    config: &crate::PredictConfig,
) -> Result<Box<dyn qingjian_core::SelectionTranslator>, crate::PredictError> {
    if !config.enabled || !config.translation.enabled {
        return Ok(Box::new(qingjian_core::NoSelectionTranslator));
    }
    match config.translation.channel {
        TranslationChannel::Llm => Ok(Box::new(cloud_translator::CloudSelectionTranslator::new(
            config,
        )?)),
    }
}

#[cfg(test)]
mod tests;
