//! 采集、编码和请求都在任务工作线程执行，运行时随任务释放。

use super::control::Control;
use super::event::Event;
use qingjian_recognize::Recognizer;
use std::sync::{Arc, mpsc::Sender};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

pub(super) fn run(
    id: u32,
    speech: bool,
    language: String,
    control: Arc<Control>,
    sender: Sender<Event>,
    hwnd: isize,
    target_seed: (isize, isize),
) {
    let post = |event| {
        if sender.send(event).is_ok() {
            let _ = unsafe {
                PostMessageW(
                    Some(HWND(hwnd as *mut _)),
                    super::WAKE,
                    WPARAM(0),
                    LPARAM(0),
                )
            };
        }
    };
    let result = (|| {
        let _apartment =
            super::apartment::Apartment::new().map_err(|_| "无法初始化识别任务".to_owned())?;
        let target = super::target::Target::capture(id, target_seed);
        if control.is_cancelled() {
            return Ok(None);
        }
        let media = if speech {
            super::audio::record(&control, || post(Event::Recording(id)))?
        } else {
            super::screen::select(control.clone())?
        };
        let Some(media) = media else {
            return Ok(None);
        };
        if control.is_cancelled() {
            return Ok(None);
        }
        post(Event::Processing(id));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "无法启动识别任务".to_owned())?;
        let recognized = runtime.block_on(async {
            let recognizer = Recognizer::new().map_err(|error| error.user_message())?;
            let recognize = async {
                if speech {
                    recognizer.speech(media, &language).await
                } else {
                    recognizer.ocr(media).await
                }
            };
            tokio::select! {
                result = recognize => result.map(Some).map_err(|error| error.user_message()),
                () = control.wait_for_cancel() => Ok(None),
            }
        })?;
        if control.is_cancelled() {
            return Ok(None);
        }
        let Some(text) = recognized else {
            return Ok(None);
        };
        if text.trim().is_empty() {
            return Ok(Some("未识别到文字，原剪贴板保持不变".to_owned()));
        }
        if !control.begin_commit() {
            return Ok(None);
        }
        if super::target::clipboard(HWND(hwnd as *mut _), &text).is_err() {
            return Err("剪贴板被其他程序占用，未能复制识别结果，请重试".to_owned());
        }
        Ok(Some(target.insert(HWND(hwnd as *mut _), &text).to_owned()))
    })();
    post(Event::Finished(id, result));
}
