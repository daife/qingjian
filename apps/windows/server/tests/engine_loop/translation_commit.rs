//! 译词上屏托底闭环：修饰键、长按高亮、分段消耗与失焦取消。

use crate::support::{
    ClientMessage, KeyEvent, Router, RouterConfig, SESSION, ServerMessage, SessionId, TRANSLATE,
    TRANSLATE_SECOND, digit_with, function_key, open_session, preedit, press, router_with, slot_of,
    type_letters,
};
use qingjian_core::{NoTranslator, Prediction, PredictionRequest, SelectionTranslator};
use std::sync::{Arc, Mutex};

type Requests = Arc<Mutex<Vec<PredictionRequest>>>;
type Reply = Arc<Mutex<Option<Prediction>>>;

struct Service {
    requests: Requests,

    reply: Reply,
}

impl SelectionTranslator for Service {
    fn submit(&mut self, request: PredictionRequest) {
        self.requests.lock().unwrap().push(request);
    }

    fn poll(&mut self) -> Option<Prediction> {
        self.reply.lock().unwrap().take()
    }
}

fn setup() -> (Router, Requests, Reply) {
    let requests = Arc::default();
    let reply = Arc::default();
    let mut router = router_with(RouterConfig {
        page_size: 1,
        ..RouterConfig::default()
    });
    let engine = router.engine_mut();
    engine.set_translator(Box::new(NoTranslator));
    engine.set_selection_translator(Box::new(Service {
        requests: Arc::clone(&requests),
        reply: Arc::clone(&reply),
    }));
    engine.set_translation_commit_fallback(true);
    (router, requests, reply)
}

fn complete(requests: &Requests, reply: &Reply, text: Option<&str>) {
    *reply.lock().unwrap() = Some(Prediction {
        sequence: requests.lock().unwrap().last().unwrap().sequence,
        words: Vec::new(),
        sentence: text.map(str::to_owned),
    });
}

fn poll(router: &mut Router) -> (Option<String>, qingjian_platform::protocol::Frame) {
    match router.handle(ClientMessage::Poll { session: SESSION }) {
        Some(ServerMessage::Update { commit, frame, .. }) => (commit, frame),
        other => panic!("expected update, got {other:?}"),
    }
}

#[test]
fn both_digit_senses_and_highlighted_hold_use_fallback_and_preserve_tail() {
    for variant in 0..3 {
        let (mut router, requests, reply) = setup();
        type_letters(&mut router, "nihao");
        let (_, _, frame) = press(&mut router, function_key(0x28));
        assert_eq!(frame.candidates.items[frame.highlight].text, "你");
        let event = if variant < 2 {
            digit_with(
                slot_of(&frame, "你"),
                if variant == 0 {
                    TRANSLATE
                } else {
                    TRANSLATE_SECOND
                },
            )
        } else {
            let mut event = KeyEvent::new(0x0d, None, Default::default());
            event.long_press = true;
            event
        };
        let (_, commit, before) = press(&mut router, event);
        assert!(commit.is_none());
        assert_eq!(preedit(&before), "ni'hao");
        assert_eq!(requests.lock().unwrap()[0].text, "你");
        complete(&requests, &reply, Some("you"));
        let (commit, after) = poll(&mut router);
        assert_eq!(commit.as_deref(), Some("you"));
        assert_eq!(preedit(&after), "hao");
        assert!(poll(&mut router).0.is_none());
    }
}

#[test]
fn more_input_failure_and_focus_change_do_not_insert_late_translation() {
    for variant in 0..3 {
        let (mut router, requests, reply) = setup();
        let (_, _, frame) = type_letters(&mut router, "nihao");
        press(&mut router, digit_with(slot_of(&frame, "你好"), TRANSLATE));
        match variant {
            0 => {
                type_letters(&mut router, "ma");
            }
            1 => {}
            _ => {
                let other = SessionId(2);
                open_session(&mut router, other, Some("other".into()));
                router.handle(ClientMessage::Key {
                    session: other,
                    event: KeyEvent::new(0x41, Some('a'), Default::default()),
                });
            }
        }
        complete(
            &requests,
            &reply,
            if variant == 1 { None } else { Some("stale") },
        );
        let (commit, after) = poll(&mut router);
        assert!(commit.is_none());
        if variant == 1 {
            assert_eq!(preedit(&after), "ni'hao");
        }
        assert!(!router.engine_mut().translation_commit_pending());
    }
}

#[test]
fn old_dll_cannot_start_a_fallback_it_cannot_commit() {
    let (mut router, requests, _) = setup();
    router.handle(ClientMessage::OpenSession {
        session: SESSION,
        app: None,
        protocol: 7,
    });
    let (_, _, frame) = type_letters(&mut router, "nihao");
    let (_, commit, after) = press(&mut router, digit_with(slot_of(&frame, "你好"), TRANSLATE));
    assert!(commit.is_none());
    assert!(requests.lock().unwrap().is_empty());
    assert!(after.notice.as_deref().unwrap().contains("重新打开"));
}
