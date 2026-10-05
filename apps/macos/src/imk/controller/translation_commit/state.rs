//! 异步译词上屏轮询期间持有原输入会话，避免把结果送给其他文本框。

use objc2::rc::Retained;
use objc2::runtime::AnyObject;

use crate::imk::QingjianInputController;

pub(super) struct CommitTarget {
    pub controller: Retained<QingjianInputController>,

    pub client: Retained<AnyObject>,

    pub sequence: u64,
}
