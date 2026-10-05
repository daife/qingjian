//! 译词上屏托底的原生轮询入口。

mod state;
mod ticker;

pub(super) use ticker::await_translation_commit;
