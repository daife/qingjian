//! 从协议按键验证按页联想、请求内容、云端词布局与组句历史边界。

use qingjian_core::{
    CloudWord, Engine, Prediction, PredictionPolicy, PredictionRequest, Predictor,
};
use qingjian_dictionary::Dictionary;
use qingjian_platform::protocol::{
    ClientMessage, Frame, KeyEvent, KeyModifiers, PROTOCOL_VERSION, ServerMessage, SessionId,
};
use qingjian_windows_server::{Router, RouterConfig};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Recording {
    requests: Vec<PredictionRequest>,
    replies: Vec<Prediction>,
}
struct FakePredictor(Arc<Mutex<Recording>>);
impl Predictor for FakePredictor {
    fn policy(&self) -> PredictionPolicy {
        PredictionPolicy {
            slots: 1,
            max_items: 1,
            sentence: false,
            ..PredictionPolicy::default()
        }
    }
    fn submit(&mut self, request: PredictionRequest) {
        self.0.lock().unwrap().requests.push(request);
    }
    fn poll(&mut self) -> Option<Prediction> {
        self.0.lock().unwrap().replies.pop()
    }
}
fn setup() -> (Router, Arc<Mutex<Recording>>) {
    let state = Arc::new(Mutex::new(Recording::default()));
    let dictionary = "你尼呢泥妮倪逆拟腻溺匿睨"
        .chars()
        .enumerate()
        .map(|(i, c)| format!("{c}\tni\t{}\n", 100 - i))
        .collect::<String>();
    let engine = Engine::new(Dictionary::parse(&dictionary).unwrap())
        .with_predictor(Box::new(FakePredictor(state.clone())));
    let mut router = Router::new(
        engine,
        RouterConfig {
            page_size: 3,
            cloud_slots: 1,
            ..RouterConfig::default()
        },
    );
    router.handle(ClientMessage::OpenSession {
        session: SessionId(1),
        app: None,
        protocol: PROTOCOL_VERSION,
    });
    (router, state)
}
fn key(router: &mut Router, code: u32, ch: Option<char>) -> Frame {
    match router
        .handle(ClientMessage::Key {
            session: SessionId(1),
            event: KeyEvent::new(code, ch, KeyModifiers::default()),
        })
        .unwrap()
    {
        ServerMessage::KeyResult { frame, .. } => frame,
        _ => panic!("key result"),
    }
}
fn compose(router: &mut Router) -> Frame {
    key(router, 78, Some('n'));
    key(router, 73, Some('i'))
}
fn reply(state: &Arc<Mutex<Recording>>, text: &str) {
    let mut state = state.lock().unwrap();
    let sequence = state.requests.last().unwrap().sequence;
    state.replies.push(Prediction {
        sequence,
        words: vec![CloudWord {
            text: text.into(),
            syllables: vec!["ni".into()],
            reading: None,
        }],
        sentence: None,
    });
}

#[test]
fn paging_sends_visible_prefix_and_this_compositions_guesses() {
    let (mut router, state) = setup();
    let first = compose(&mut router);
    assert_eq!(
        state.lock().unwrap().requests.last().unwrap().candidates,
        first
            .candidates
            .items
            .iter()
            .map(|c| c.text.clone())
            .collect::<Vec<_>>()
    );
    reply(&state, "祢");
    let first = key(&mut router, 0x28, None); // 同页移动只收结果，不重发。
    assert_eq!(first.candidates.items.last().unwrap().text, "祢");
    let count = state.lock().unwrap().requests.len();
    let second = key(&mut router, 0x22, None);
    let request = state.lock().unwrap().requests.last().unwrap().clone();
    assert_eq!(state.lock().unwrap().requests.len(), count + 1);
    assert_eq!(request.max_items, 1);
    let expected = first
        .candidates
        .items
        .iter()
        .chain(&second.candidates.items)
        .map(|c| c.text.clone())
        .collect::<Vec<_>>();
    assert_eq!(request.candidates, expected);
    assert_eq!(request.candidates.len(), 6); // 超过原先的五条截断。
    reply(&state, "伲");
    let second = key(&mut router, 0x28, None);
    assert_eq!(second.page, 1);
    assert_eq!(second.candidates.items.last().unwrap().text, "伲");
    let first_again = key(&mut router, 0x21, None);
    assert_eq!(first_again.candidates.items.last().unwrap().text, "祢");
    let request = state.lock().unwrap().requests.last().unwrap().clone();
    assert!(request.candidates.contains(&"伲".to_owned())); // 后一页猜词仍属于本轮历史。
    assert!(request.candidates.contains(&"祢".to_owned()));
    key(&mut router, 0x1b, None);
    compose(&mut router);
    let request = state.lock().unwrap().requests.last().unwrap().clone();
    assert!(!request.candidates.iter().any(|c| c == "祢" || c == "伲"));
}

#[test]
fn arrow_navigation_requests_on_page_crossing_and_discards_old_page_results() {
    let (mut router, state) = setup();
    compose(&mut router);
    reply(&state, "祢");
    let count = state.lock().unwrap().requests.len();
    key(&mut router, 0x22, None); // 上一页仍在飞的结果不能落到新页。
    assert_eq!(state.lock().unwrap().requests.len(), count + 1);
    key(&mut router, 0x26, None); // 第二页第一项向上，回到第一页末项。
    assert_eq!(state.lock().unwrap().requests.len(), count + 2);
    let frame = key(&mut router, 0x28, None); // 再次跨到第二页。
    assert_eq!(frame.page, 1);
    assert_eq!(state.lock().unwrap().requests.len(), count + 3);
    assert!(
        !state
            .lock()
            .unwrap()
            .requests
            .last()
            .unwrap()
            .candidates
            .contains(&"祢".to_owned())
    );
}
