//! 有道公开网页的 OCR / ASR；只接受已编码的图片与 WAV，不涉及系统采集或输入引擎。

mod client;
mod error;

pub use client::Recognizer;
pub use error::RecognitionError;

pub const MAX_AUDIO_SECONDS: u32 = 60;
pub const AUDIO_RATE: u32 = 16_000;
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
