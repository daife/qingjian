//! DLL 上下文必须进入真正提交给联想器的请求，并在上屏后清除。

use std::sync::{Arc, Mutex};

use qingjian_core::{Engine, Prediction, PredictionPolicy, PredictionRequest, Predictor};
use qingjian_dictionary::Dictionary;
use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, SessionId,
};
use qingjian_windows_server::{Router, RouterConfig};

struct RecordingPredictor(Arc<Mutex<Vec<PredictionRequest>>>);

impl Predictor for RecordingPredictor {
    fn policy(&self) -> PredictionPolicy {
        PredictionPolicy::default()
    }

    fn submit(&mut self, request: PredictionRequest) {
        self.0.lock().unwrap().push(request);
    }

    fn poll(&mut self) -> Option<Prediction> {
        None
    }
}

fn type_keys(router: &mut Router, keys: &str) {
    for ch in keys.chars() {
        router.handle(ClientMessage::Key {
            session: SessionId(1),
            event: KeyEvent::new(ch as u32, Some(ch), KeyModifiers::default()),
        });
    }
}

#[test]
fn dll_context_replaces_contextless_request_and_does_not_leak_to_next_composition() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let engine = Engine::new(Dictionary::parse("你\tni\t100\n").unwrap())
        .with_predictor(Box::new(RecordingPredictor(requests.clone())));
    let mut router = Router::new(engine, RouterConfig::default());
    router.handle(ClientMessage::OpenSession {
        session: SessionId(1),
        app: None,
        protocol: PROTOCOL_VERSION,
    });
    type_keys(&mut router, "ni");
    let initial = requests.lock().unwrap().last().unwrap().clone();
    assert!(initial.before.is_empty());

    router.handle(ClientMessage::Surrounding {
        session: SessionId(1),
        text: "我们正在讨论".to_owned(),
    });
    let refreshed = requests.lock().unwrap().last().unwrap().clone();
    assert!(refreshed.sequence > initial.sequence);
    assert_eq!(refreshed.before, "我们正在讨论");
    assert!(refreshed.after.is_empty());

    router.handle(ClientMessage::Surrounding {
        session: SessionId(99),
        text: "另一个应用".to_owned(),
    });
    assert_eq!(
        requests.lock().unwrap().last().unwrap().sequence,
        refreshed.sequence
    );

    type_keys(&mut router, " ");
    type_keys(&mut router, "ni");
    assert!(requests.lock().unwrap().last().unwrap().before.is_empty());
}
