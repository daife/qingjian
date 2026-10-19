//! 工作线程回报的任务进度；任务号用于拒绝取消后的迟到结果。

pub(super) enum Event {
    Recording(u32),

    Processing(u32),

    Finished(u32, Result<Option<String>, String>),
}
