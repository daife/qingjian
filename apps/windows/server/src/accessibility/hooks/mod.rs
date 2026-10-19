//! 临时低级钩子；只在任务期间消费结束 / 取消操作。

mod state;

use self::state::HookState;
use crate::accessibility::control::Control;
use std::cell::RefCell;
use std::sync::Arc;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HHOOK, KBDLLHOOKSTRUCT, LLKHF_INJECTED, LLMHF_INJECTED, MSLLHOOKSTRUCT,
    SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL,
};

thread_local! {
    static STATE: RefCell<Option<HookState>> = const { RefCell::new(None) };
}

pub(super) struct Hooks {
    keyboard: HHOOK,

    mouse: HHOOK,
}

impl Hooks {
    pub(super) fn new(control: Arc<Control>, recording: bool) -> windows::core::Result<Self> {
        let keyboard = unsafe {
            SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard),
                Some(crate::accessibility::module()),
                0,
            )?
        };
        let mouse = match unsafe {
            SetWindowsHookExW(
                WH_MOUSE_LL,
                Some(mouse),
                Some(crate::accessibility::module()),
                0,
            )
        } {
            Ok(mouse) => mouse,
            Err(error) => {
                let _ = unsafe { UnhookWindowsHookEx(keyboard) };
                return Err(error);
            }
        };
        STATE.with(|state| *state.borrow_mut() = Some(HookState::new(control, recording)));
        Ok(Self { keyboard, mouse })
    }

    pub(super) fn processing(&self) {
        STATE.with(|state| {
            if let Some(state) = state.borrow_mut().as_mut() {
                state.processing();
            }
        });
    }

    pub(super) fn drain(&self) -> bool {
        STATE.with(|state| state.borrow_mut().as_mut().is_none_or(HookState::drain))
    }
}

impl Drop for Hooks {
    fn drop(&mut self) {
        STATE.with(|state| state.borrow_mut().take());
        unsafe {
            let _ = UnhookWindowsHookEx(self.keyboard);
            let _ = UnhookWindowsHookEx(self.mouse);
        }
    }
}

unsafe extern "system" fn keyboard(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let key = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if !key.flags.contains(LLKHF_INJECTED)
            && STATE.with(|state| {
                state
                    .try_borrow_mut()
                    .ok()
                    .and_then(|mut state| {
                        state
                            .as_mut()
                            .map(|state| state.keyboard(key.vkCode, wparam.0 as u32))
                    })
                    .unwrap_or(false)
            })
        {
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn mouse(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0
        && unsafe { (*(lparam.0 as *const MSLLHOOKSTRUCT)).flags } & LLMHF_INJECTED == 0
        && STATE.with(|state| {
            state
                .try_borrow_mut()
                .ok()
                .and_then(|mut state| state.as_mut().map(|state| state.mouse(wparam.0 as u32)))
                .unwrap_or(false)
        })
    {
        return LRESULT(1);
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
