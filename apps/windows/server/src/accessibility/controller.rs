//! 热键、任务生命周期与不抢焦点的状态提示。

use super::{control::Control, event::Event, hooks::Hooks, job::Job, target::Target};
use qingjian_platform::{Config, KeyCombo};
use std::sync::{
    Arc,
    mpsc::{Receiver, Sender},
};
use std::time::{Duration, Instant, SystemTime};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey,
    UnregisterHotKey,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GetCursorPos, HWND_TOPMOST, SW_HIDE, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, SetTimer, SetWindowPos, SetWindowTextW, ShowWindow, WS_BORDER,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, w};

pub(super) struct Controller {
    hwnd: HWND,

    popup: HWND,

    config: Config,

    stamp: Option<(SystemTime, u64)>,

    loaded: bool,

    last_reload: Instant,

    registered: [bool; 2],

    job: Option<Job>,

    hooks: Option<Hooks>,

    sequence: u32,

    sender: Sender<Event>,

    receiver: Receiver<Event>,

    notice_until: Option<Instant>,
}

impl Controller {
    pub(super) fn new(
        hwnd: HWND,
        sender: Sender<Event>,
        receiver: Receiver<Event>,
    ) -> windows::core::Result<Self> {
        let popup = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!(""),
                WS_POPUP | WS_BORDER,
                0,
                0,
                640,
                56,
                None,
                None,
                Some(super::module()),
                None,
            )?
        };
        Ok(Self {
            hwnd,
            popup,
            config: Config::default(),
            stamp: None,
            loaded: false,
            last_reload: Instant::now(),
            registered: [false; 2],
            job: None,
            hooks: None,
            sequence: (SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u32)
                .max(1),
            sender,
            receiver,
            notice_until: None,
        })
    }

    pub(super) fn reload(&mut self) {
        self.last_reload = Instant::now();
        let Some(path) = qingjian_platform::dirs::config_path() else {
            return;
        };
        let stamp = std::fs::metadata(&path)
            .ok()
            .and_then(|metadata| Some((metadata.modified().ok()?, metadata.len())));
        if self.loaded && self.stamp == stamp {
            return;
        }
        let config = match Config::load(&path) {
            Ok(config) => config,
            Err(_) => {
                self.stamp = stamp;
                self.loaded = true;
                return;
            }
        };
        self.stamp = stamp;
        self.loaded = true;
        if self.config.accessibility != config.accessibility
            && let Some(job) = &self.job
        {
            job.control.cancel();
        }
        let mut errors = Vec::new();
        for (index, (enabled, combo, label, channel)) in [
            (
                config.accessibility.ocr_enabled,
                config.shortcut.screenshot_ocr,
                "截图 OCR",
                config.accessibility.ocr_channel.as_str(),
            ),
            (
                config.accessibility.speech_enabled,
                config.shortcut.speech_recognition,
                "语音识别",
                config.accessibility.speech_channel.as_str(),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            if self.registered[index] {
                let _ = unsafe { UnregisterHotKey(Some(self.hwnd), index as i32 + 1) };
            }
            self.registered[index] = false;
            if !enabled || combo.modifiers.is_empty() {
                continue;
            }
            if combo == config.shortcut.translate_selection {
                errors.push(format!("{label} 快捷键与「翻译选中文字」冲突，请更换组合"));
                continue;
            }
            if index == 1
                && config.accessibility.ocr_enabled
                && !config.shortcut.screenshot_ocr.modifiers.is_empty()
                && combo == config.shortcut.screenshot_ocr
            {
                errors.push("语音识别快捷键与截图 OCR 冲突，请更换组合".to_owned());
                continue;
            }
            if channel != "youdao_public" {
                errors.push(format!("{label}：不支持的识别渠道，请在设置中重新选择"));
                continue;
            }
            match unsafe {
                RegisterHotKey(
                    Some(self.hwnd),
                    index as i32 + 1,
                    modifiers(combo),
                    combo.key.to_ascii_uppercase() as u32,
                )
            } {
                Ok(()) => self.registered[index] = true,
                Err(error) => {
                    errors.push(format!(
                        "{label} 快捷键 {combo} 注册失败（可能已占用），请在「快捷键」页更换"
                    ));
                    tracing::warn!(code = error.code().0, %combo, "辅助功能快捷键注册失败");
                }
            }
        }
        self.config = config;
        if !errors.is_empty() && self.job.is_none() {
            self.notice(&errors.join("；"));
        }
    }

    pub(super) fn start(&mut self, speech: bool) {
        if self.job.is_some() || self.hooks.is_some() {
            return;
        }
        if !self.registered[usize::from(speech)] {
            return;
        }
        self.sequence = self.sequence.wrapping_add(1).max(1);
        let control = match Control::new() {
            Ok(control) => Arc::new(control),
            Err(_) => {
                self.notice("无法启动辅助功能任务");
                return;
            }
        };
        let target = Target::seed();
        let hooks = match Hooks::new(control.clone(), speech) {
            Ok(hooks) => hooks,
            Err(_) => {
                self.notice("无法监听取消操作，辅助功能未启动");
                return;
            }
        };
        self.hooks = Some(hooks);
        self.job = Some(Job {
            id: self.sequence,
            control: control.clone(),
            recording_since: None,
        });
        self.notice_until = None;
        if speech {
            self.show("正在打开麦克风…  Esc / 右键取消");
        } else {
            unsafe {
                let _ = ShowWindow(self.popup, SW_HIDE);
            }
        }
        unsafe {
            SetTimer(Some(self.hwnd), 1, 100, None);
        }
        let (id, sender, hwnd, language) = (
            self.sequence,
            self.sender.clone(),
            self.hwnd.0 as isize,
            self.config.accessibility.speech_language.clone(),
        );
        if std::thread::Builder::new()
            .name("qingjian-recognition".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    super::worker::run(id, speech, language, control, sender.clone(), hwnd, target)
                }));
                if result.is_err() {
                    let _ =
                        sender.send(Event::Finished(id, Err("识别任务意外终止，请重试".into())));
                    let _ = unsafe {
                        windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                            Some(HWND(hwnd as *mut _)),
                            super::WAKE,
                            Default::default(),
                            Default::default(),
                        )
                    };
                }
            })
            .is_err()
        {
            self.job.take();
            self.notice("无法启动识别工作线程");
        }
    }

    pub(super) fn tick(&mut self) {
        if self.last_reload.elapsed() >= Duration::from_secs(1) {
            self.reload();
        }
        self.events();
        if let Some(job) = &self.job {
            if job.control.is_cancelled() {
                self.show("正在取消…");
            } else if let Some(start) = job.recording_since {
                if job.control.finish_requested() {
                    self.show("正在结束录音并识别…  Esc / 右键取消");
                } else {
                    self.show(&format!(
                        "正在录音  {:02} / 60 秒 · 空格结束识别 · Esc / 右键取消",
                        start.elapsed().as_secs().min(60)
                    ));
                }
            }
        } else if self.hooks.as_ref().is_none_or(Hooks::drain) {
            self.hooks.take();
            unsafe {
                SetTimer(Some(self.hwnd), 1, 1000, None);
            }
        }
        if self
            .notice_until
            .is_some_and(|until| Instant::now() >= until)
        {
            unsafe {
                let _ = ShowWindow(self.popup, SW_HIDE);
            }
            self.notice_until = None;
        }
    }

    pub(super) fn events(&mut self) {
        while let Ok(event) = self.receiver.try_recv() {
            let id = match &event {
                Event::Recording(id) | Event::Processing(id) | Event::Finished(id, _) => *id,
            };
            if self.job.as_ref().is_none_or(|job| job.id != id) {
                continue;
            }
            match event {
                Event::Recording(_) => {
                    if let Some(job) = &mut self.job {
                        job.recording_since = Some(Instant::now());
                    }
                }
                Event::Processing(_) => {
                    if let Some(job) = &mut self.job {
                        job.recording_since = None;
                    }
                    if let Some(hooks) = &self.hooks {
                        hooks.processing();
                    }
                    self.show("正在识别…  Esc / 右键取消");
                }
                Event::Finished(_, result) => {
                    let Some(job) = self.job.take() else {
                        continue;
                    };
                    if job.control.is_cancelled() {
                        self.notice("已取消");
                        continue;
                    }
                    match result {
                        Ok(Some(notice)) => self.notice(&notice),
                        Ok(None) => self.notice("已取消"),
                        Err(error) => {
                            tracing::warn!("辅助识别失败：{error}");
                            self.notice(&error);
                        }
                    }
                }
            }
        }
    }

    fn show(&self, text: &str) {
        let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let mut point = POINT::default();
        let _ = unsafe { GetCursorPos(&mut point) };
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        unsafe {
            let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
            if !GetMonitorInfoW(monitor, &mut info).as_bool() {
                return;
            }
            let area = info.rcWork;
            let width = 640.min(area.right - area.left);
            let _ = SetWindowTextW(self.popup, PCWSTR(text.as_ptr()));
            let _ = SetWindowPos(
                self.popup,
                Some(HWND_TOPMOST),
                area.left + (area.right - area.left - width) / 2,
                area.bottom - 100,
                width,
                56,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }

    fn notice(&mut self, text: &str) {
        self.notice_until = Some(Instant::now() + Duration::from_secs(6));
        self.show(text);
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        if let Some(job) = &self.job {
            job.control.cancel();
        }
        for (index, registered) in self.registered.iter().enumerate() {
            if *registered {
                let _ = unsafe { UnregisterHotKey(Some(self.hwnd), index as i32 + 1) };
            }
        }
        let _ = unsafe { DestroyWindow(self.popup) };
    }
}

fn modifiers(combo: KeyCombo) -> HOT_KEY_MODIFIERS {
    let mut modifiers = MOD_NOREPEAT;
    if combo.modifiers.control {
        modifiers |= MOD_CONTROL;
    }
    if combo.modifiers.shift {
        modifiers |= MOD_SHIFT;
    }
    if combo.modifiers.option {
        modifiers |= MOD_ALT;
    }
    if combo.modifiers.command {
        modifiers |= MOD_WIN;
    }
    modifiers
}
