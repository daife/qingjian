//! 译词上屏托底的原生轮询：Core 返回译文后写入原会话并刷新剩余输入。

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_foundation::{NSObject, NSObjectProtocol, NSTimer};

use super::state::CommitTarget;
use crate::host;
use crate::imk::{QingjianInputController, TextClient};

pub(in crate::imk::controller) fn await_translation_commit(
    controller: &QingjianInputController,
    client: TextClient<'_>,
) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(sequence) = host::with(|h| h.engine.translation_commit_sequence()).flatten() else {
        return;
    };
    let target = TranslationCommitTicker::alloc(mtm).set_ivars(CommitTarget {
        sequence,
        controller: controller.retain(),
        client: client.object().retain(),
    });
    let target: Retained<TranslationCommitTicker> = unsafe { msg_send![super(target), init] };
    unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
            0.05,
            &target,
            sel!(tick:),
            None,
            true,
        );
    }
}

define_class!(
    // SAFETY: NSObject 无子类化要求；回调和 ivars 只在主线程访问。
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = CommitTarget]
    struct TranslationCommitTicker;

    impl TranslationCommitTicker {
        #[unsafe(method(tick:))]
        fn tick(&self, timer: &NSTimer) {
            let result = host::with(|h| {
                if h.engine.translation_commit_sequence() != Some(self.ivars().sequence) {
                    return (None, false);
                }
                let text = h.engine.poll_translation_commit();
                (text, h.engine.translation_commit_pending())
            });
            let Some((text, pending)) = result else { timer.invalidate(); return; };
            if let Some(text) = text {
                let client = TextClient::new(&self.ivars().client);
                client.insert_text(&text);
                self.ivars().controller.refresh(client);
            }
            if !pending { timer.invalidate(); }
        }
    }

    unsafe impl NSObjectProtocol for TranslationCommitTicker {}
);
