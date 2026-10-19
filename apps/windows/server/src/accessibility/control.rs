//! 跨线程录音结束与任务取消信号；事件只在任务期间分配。

use std::sync::atomic::{AtomicU8, Ordering};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Threading::{CreateEventW, SetEvent};
use windows::core::Result;

const RUNNING: u8 = 0;
const FINISH_REQUESTED: u8 = 1;
const CANCELLED: u8 = 2;
const DELIVERING: u8 = 3;

pub(super) struct Control {
    event: isize,

    action: AtomicU8,

    cancellation: tokio::sync::Notify,
}

impl Control {
    pub(super) fn new() -> Result<Self> {
        let event = unsafe { CreateEventW(None, true, false, None)? };
        Ok(Self {
            event: event.0 as isize,
            action: AtomicU8::new(RUNNING),
            cancellation: tokio::sync::Notify::new(),
        })
    }

    pub(super) fn event(&self) -> HANDLE {
        HANDLE(self.event as *mut _)
    }

    pub(super) fn finish(&self) {
        let _ = self.action.compare_exchange(
            RUNNING,
            FINISH_REQUESTED,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        let _ = unsafe { SetEvent(self.event()) };
    }

    pub(super) fn cancel(&self) {
        // 结果开始交付后不再把已复制 / 已排入 TSF 的任务标成取消。
        if self
            .action
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |action| {
                (action != DELIVERING).then_some(CANCELLED)
            })
            .is_err()
        {
            return;
        }
        let _ = unsafe { SetEvent(self.event()) };
        self.cancellation.notify_one();
    }

    pub(super) fn begin_commit(&self) -> bool {
        self.action
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |action| {
                (action == RUNNING || action == FINISH_REQUESTED).then_some(DELIVERING)
            })
            .is_ok()
    }

    pub(super) fn action(&self) -> u8 {
        self.action.load(Ordering::Acquire)
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.action() == CANCELLED
    }

    pub(super) fn finish_requested(&self) -> bool {
        self.action() == FINISH_REQUESTED
    }

    pub(super) fn is_running(&self) -> bool {
        self.action() == RUNNING
    }

    pub(super) async fn wait_for_cancel(&self) {
        while !self.is_cancelled() {
            self.cancellation.notified().await;
        }
    }
}

impl Drop for Control {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.event()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_wins_before_delivery_and_cannot_report_delivered_results_as_cancelled() {
        let cancelled = Control::new().unwrap();
        cancelled.finish();
        cancelled.cancel();
        assert!(!cancelled.begin_commit());
        assert!(cancelled.is_cancelled());
        let delivered = Control::new().unwrap();
        delivered.finish();
        assert!(delivered.begin_commit());
        delivered.cancel();
        assert!(!delivered.is_cancelled());
    }

    #[test]
    fn network_cancellation_wakes_before_or_after_waiter_registration() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let early = Control::new().unwrap();
            early.cancel();
            tokio::time::timeout(std::time::Duration::from_secs(1), early.wait_for_cancel())
                .await
                .unwrap();
            let late = std::sync::Arc::new(Control::new().unwrap());
            let waiter = late.clone();
            let task = tokio::spawn(async move {
                waiter.wait_for_cancel().await;
            });
            tokio::task::yield_now().await;
            late.cancel();
            tokio::time::timeout(std::time::Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap();
        });
    }
}
