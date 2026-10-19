//! 消费过的按下必须配对消费松开；任务结束后仅排空旧事件。

use crate::accessibility::control::Control;
use std::sync::Arc;
use windows::Win32::UI::WindowsAndMessaging::{
    WM_KEYDOWN, WM_KEYUP, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

const SPACE: u8 = 1;
const ESCAPE: u8 = 2;
const RIGHT_MOUSE: u8 = 4;

pub(super) struct HookState {
    control: Arc<Control>,

    recording: bool,

    accepting: bool,

    consumed: u8,
}

impl HookState {
    pub(super) fn new(control: Arc<Control>, recording: bool) -> Self {
        Self {
            control,
            recording,
            accepting: true,
            consumed: 0,
        }
    }

    pub(super) fn processing(&mut self) {
        self.recording = false;
    }

    pub(super) fn drain(&mut self) -> bool {
        self.accepting = false;
        self.consumed == 0
    }

    pub(super) fn keyboard(&mut self, key: u32, message: u32) -> bool {
        let bit = match key {
            0x20 => SPACE,
            0x1B => ESCAPE,
            _ => return false,
        };
        if [WM_KEYUP, WM_SYSKEYUP].contains(&message) && self.consumed & bit != 0 {
            self.consumed &= !bit;
            return true;
        }
        if [WM_KEYDOWN, WM_SYSKEYDOWN].contains(&message)
            && (self.consumed & bit != 0 || (self.accepting && (bit == ESCAPE || self.recording)))
        {
            self.consumed |= bit;
            if bit == ESCAPE {
                self.control.cancel();
            } else {
                self.control.finish();
                self.recording = false;
            }
            return true;
        }
        false
    }

    pub(super) fn mouse(&mut self, message: u32) -> bool {
        match message {
            WM_RBUTTONDOWN if self.accepting || self.consumed & RIGHT_MOUSE != 0 => {
                self.consumed |= RIGHT_MOUSE;
                self.control.cancel();
                true
            }
            WM_RBUTTONUP if self.consumed & RIGHT_MOUSE != 0 => {
                self.consumed &= !RIGHT_MOUSE;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ending_recording_swallows_repeat_and_release_after_completion() {
        let control = Arc::new(Control::new().unwrap());
        let mut state = HookState::new(control.clone(), true);
        assert!(state.keyboard(0x20, WM_KEYDOWN));
        assert_eq!(control.action(), 1);
        assert!(!state.drain());
        assert!(state.keyboard(0x20, WM_KEYDOWN));
        assert!(state.keyboard(0x20, WM_KEYUP));
        assert!(state.drain());
        assert!(!state.keyboard(0x20, WM_KEYDOWN));
        assert!(!state.keyboard(0x20, WM_KEYUP));
    }

    #[test]
    fn right_cancel_does_not_leak_release_or_start_another_operation() {
        let control = Arc::new(Control::new().unwrap());
        let mut state = HookState::new(control.clone(), false);
        assert!(!state.keyboard(0x20, WM_KEYDOWN));
        assert!(state.mouse(WM_RBUTTONDOWN));
        assert_eq!(control.action(), 2);
        assert!(!state.drain());
        assert!(state.mouse(WM_RBUTTONUP));
        assert!(state.drain());
        assert!(!state.mouse(WM_RBUTTONDOWN));
    }

    #[test]
    fn escape_cancels_after_space_has_requested_recognition() {
        let control = Arc::new(Control::new().unwrap());
        let mut state = HookState::new(control.clone(), true);
        assert!(state.keyboard(0x20, WM_KEYDOWN));
        assert!(state.keyboard(0x20, WM_KEYUP));
        state.processing();
        assert!(!state.keyboard(0x20, WM_KEYDOWN));
        assert!(state.keyboard(0x1B, WM_KEYDOWN));
        assert_eq!(control.action(), 2);
        assert!(state.keyboard(0x1B, WM_KEYUP));
    }
}
