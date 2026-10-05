//! 一次等待松开的固定按键，保存按下时的事件与上下文。

use qingjian_platform::protocol::KeyEvent;
use std::time::Instant;
use windows::Win32::UI::TextServices::ITfContext;

pub(super) struct Press {
    pub event: KeyEvent,

    pub context: Option<ITfContext>,

    pub started: Instant,

    pub cancelled: bool,
}
