//! 消息线程持有的任务状态；COM 编辑目标和媒体归工作线程。

use super::control::Control;
use std::sync::Arc;
use std::time::Instant;

pub(super) struct Job {
    pub(super) id: u32,

    pub(super) control: Arc<Control>,

    pub(super) recording_since: Option<Instant>,
}
