//! 辅助功能目标令牌；跨进程消息只负责受理，实际插入在异步 TSF 编辑会话内完成。

use super::{TextService_Impl, with_active};
use crate::com::context::keyboard_disabled;
use std::cell::RefCell;
use windows::Win32::UI::TextServices::{ITfContext, TF_SD_READONLY};
use windows::core::Interface;

thread_local! {
    static TARGET: RefCell<Option<(u32, ITfContext)>> = const { RefCell::new(None) };
}

fn current(service: &TextService_Impl) -> Option<ITfContext> {
    let manager = service.thread_mgr.borrow();
    let manager = manager.as_ref()?;
    if !unsafe { manager.IsThreadFocus() }.ok()?.as_bool() || service.shared.composing() {
        return None;
    }
    let context = unsafe { manager.GetFocus().and_then(|document| document.GetTop()) }.ok()?;
    if keyboard_disabled(&context) {
        return None;
    }
    let status = unsafe { context.GetStatus() }.ok()?;
    if status.dwStaticFlags & TF_SD_READONLY != 0 {
        return None;
    }
    Some(context)
}

pub(crate) fn capture(token: u32) -> bool {
    let mut target = None;
    with_active(|service| {
        target = current(service).map(|context| (token, context));
    });
    let captured = target.is_some();
    TARGET.with(|slot| *slot.borrow_mut() = target);
    captured
}

pub(crate) fn release(token: Option<u32>) {
    TARGET.with(|slot| {
        let mut target = slot.borrow_mut();
        if token.is_none()
            || target
                .as_ref()
                .is_some_and(|(expected, _)| Some(*expected) == token)
        {
            target.take();
        }
    });
}

pub(crate) fn matches(context: &ITfContext) -> bool {
    let mut matches = false;
    with_active(|service| {
        matches = current(service).is_some_and(|current| current.as_raw() == context.as_raw());
    });
    matches
}

pub(crate) fn insert(token: u32, text: String) -> bool {
    let target = TARGET.with(|slot| slot.borrow_mut().take());
    let Some((expected, context)) = target else {
        return false;
    };
    if token != expected || !matches(&context) {
        return false;
    }
    let mut accepted = false;
    with_active(|service| {
        accepted =
            crate::com::edit::recognition::request(&context, service.client_id.get(), text).is_ok();
    });
    accepted
}
