//! 固定回车 / 空格的按住状态：定时触发长按，松开只触发短按。

use std::cell::RefCell;
use std::time::{Duration, Instant};

use qingjian_platform::protocol::KeyEvent;
use windows::Win32::UI::TextServices::ITfContext;

#[derive(Default)]
pub(crate) struct ReleaseKeys {
    pending: RefCell<[Option<Press>; 2]>,
}

mod press;

use press::Press;

fn slot(vk: u32) -> Option<usize> {
    match vk {
        0x0d => Some(0),
        0x20 => Some(1),
        _ => None,
    }
}

fn short_press(started: Instant, now: Instant, threshold: Duration, cancelled: bool) -> bool {
    !cancelled && now.duration_since(started) < threshold
}

impl ReleaseKeys {
    pub(crate) fn held(&self, vk: u32) -> bool {
        slot(vk).is_some_and(|i| self.pending.borrow()[i].is_some())
    }

    /// 中途敲其他键不能把后来改变的候选 / 缓冲区误当作这次短按的目标。
    pub(crate) fn note_down(&self, vk: u32) {
        for press in self.pending.borrow_mut().iter_mut().flatten() {
            if press.event.virtual_key != vk {
                press.cancelled = true;
            }
        }
    }

    pub(crate) fn begin(&self, event: KeyEvent, context: ITfContext, repeat: bool) {
        self.begin_at(event, Some(context), repeat, Instant::now());
    }

    fn begin_at(&self, event: KeyEvent, context: Option<ITfContext>, repeat: bool, now: Instant) {
        let Some(i) = slot(event.virtual_key) else {
            return;
        };
        let mut pending = self.pending.borrow_mut();
        if pending[i].is_none() {
            pending[i] = Some(Press {
                event,
                context,
                started: now,
                cancelled: repeat,
            });
        }
    }

    pub(crate) fn cancel(&self) {
        for press in self.pending.borrow_mut().iter_mut().flatten() {
            press.cancelled = true;
        }
    }

    pub(crate) fn finish(
        &self,
        vk: u32,
        context: &ITfContext,
        threshold_ms: u64,
    ) -> Option<KeyEvent> {
        self.finish_at(vk, Some(context), threshold_ms, Instant::now())
    }

    fn finish_at(
        &self,
        vk: u32,
        context: Option<&ITfContext>,
        threshold_ms: u64,
        now: Instant,
    ) -> Option<KeyEvent> {
        let press = self.pending.borrow_mut()[slot(vk)?].take()?;
        if press.cancelled || press.context.as_ref() != context {
            return None;
        }
        if !short_press(
            press.started,
            now,
            Duration::from_millis(threshold_ms),
            false,
        ) {
            return None;
        }
        Some(press.event)
    }

    pub(crate) fn due(&self, threshold_ms: u64) -> Vec<(KeyEvent, ITfContext)> {
        self.due_at(threshold_ms, Instant::now())
            .into_iter()
            .filter_map(|(event, context)| context.map(|context| (event, context)))
            .collect()
    }

    fn due_at(&self, threshold_ms: u64, now: Instant) -> Vec<(KeyEvent, Option<ITfContext>)> {
        let mut events = Vec::new();
        for press in self.pending.borrow_mut().iter_mut().flatten() {
            if !press.cancelled
                && now.duration_since(press.started) >= Duration::from_millis(threshold_ms)
            {
                // 保留按住状态吞掉重复与松开；转发前标记，防止编辑会话重入再次触发。
                press.cancelled = true;
                events.push((
                    KeyEvent {
                        long_press: true,
                        ..press.event
                    },
                    press.context.clone(),
                ));
            }
        }
        events
    }

    pub(crate) fn discard(&self, vk: u32) {
        if let Some(i) = slot(vk) {
            self.pending.borrow_mut()[i] = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(vk: u32) -> KeyEvent {
        KeyEvent::new(vk, (vk == 0x20).then_some(' '), Default::default())
    }

    #[test]
    fn release_only_fires_once_and_repeat_does_not_restart_timer() {
        let keys = ReleaseKeys::default();
        let start = Instant::now();
        keys.begin_at(event(0x20), None, false, start);
        keys.begin_at(event(0x20), None, true, start + Duration::from_millis(400));
        assert!(keys.held(0x20));
        assert!(keys.held(0x20)); // TestKeyUp 不取走状态
        assert!(
            keys.finish_at(0x20, None, 500, start + Duration::from_millis(501))
                .is_none()
        );
        assert!(!keys.held(0x20));
        keys.begin_at(event(0x0d), None, false, start);
        assert!(
            keys.finish_at(0x0d, None, 500, start + Duration::from_millis(100))
                .is_some()
        );
        assert!(
            keys.finish_at(0x0d, None, 500, start + Duration::from_millis(100))
                .is_none()
        );
    }

    #[test]
    fn interrupted_and_cancelled_presses_stay_consumed_until_release() {
        let keys = ReleaseKeys::default();
        let start = Instant::now();
        for other_key in [0x41, 0x0d] {
            keys.begin_at(event(0x20), None, false, start);
            keys.note_down(other_key);
            assert!(keys.held(0x20));
            assert!(
                keys.due_at(500, start + Duration::from_millis(500))
                    .is_empty()
            );
            assert!(keys.finish_at(0x20, None, 500, start).is_none());
        }
        keys.begin_at(event(0x0d), None, false, start);
        keys.cancel();
        assert!(keys.held(0x0d));
        assert!(keys.finish_at(0x0d, None, 500, start).is_none());
        keys.begin_at(event(0x20), None, true, start);
        assert!(keys.finish_at(0x20, None, 500, start).is_none());
    }

    #[test]
    fn long_press_fires_at_threshold_once_without_release() {
        let keys = ReleaseKeys::default();
        let start = Instant::now();
        for vk in [0x0d, 0x20] {
            keys.begin_at(event(vk), None, false, start);
            assert!(
                keys.due_at(500, start + Duration::from_millis(499))
                    .is_empty()
            );
            let result = keys.due_at(500, start + Duration::from_millis(500));
            assert_eq!(result.len(), 1);
            assert!(result[0].0.long_press);
            assert_eq!(result[0].0.virtual_key, vk);
            assert_eq!(result[0].0.character, event(vk).character);
            assert!(keys.held(vk));
            keys.begin_at(event(vk), None, true, start + Duration::from_millis(550));
            assert!(
                keys.due_at(500, start + Duration::from_millis(600))
                    .is_empty()
            );
            assert!(
                keys.finish_at(vk, None, 500, start + Duration::from_millis(600))
                    .is_none()
            );
            assert!(!keys.held(vk));
            keys.begin_at(event(vk), None, false, start);
            assert!(
                keys.finish_at(vk, None, 500, start + Duration::from_millis(499))
                    .is_some()
            );
            assert!(
                keys.due_at(500, start + Duration::from_millis(500))
                    .is_empty()
            );
            keys.begin_at(event(vk), None, false, start);
            keys.cancel();
            assert!(
                keys.due_at(500, start + Duration::from_millis(600))
                    .is_empty()
            );
            assert!(
                keys.finish_at(vk, None, 500, start + Duration::from_millis(600))
                    .is_none()
            );
        }
    }

    #[test]
    fn threshold_and_cancellation() {
        let start = Instant::now();
        let threshold = Duration::from_millis(500);
        assert!(!short_press(start, start + threshold, threshold, false));
        assert!(!short_press(
            start,
            start + threshold + Duration::from_millis(1),
            threshold,
            false
        ));
        assert!(!short_press(
            start,
            start + Duration::from_millis(10),
            threshold,
            true
        ));
    }
}
