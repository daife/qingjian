//! Windows 辅助功能独立消息线程；空闲仅注册热键并跟随配置热加载。

mod apartment;
mod audio;
mod control;
mod controller;
mod event;
mod hooks;
mod job;
mod screen;
mod target;
mod worker;

use self::controller::Controller;
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG,
    RegisterClassExW, SetTimer, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_HOTKEY,
    WM_TIMER, WNDCLASSEXW,
};
use windows::core::w;

const WAKE: u32 = WM_APP + 1;

thread_local! {
    static CONTROLLER: RefCell<Option<Controller>> = const { RefCell::new(None) };
}

pub fn spawn() {
    if let Err(error) = std::thread::Builder::new()
        .name("qingjian-accessibility".into())
        .spawn(|| {
            if let Err(error) = run() {
                tracing::error!(code = error.code().0, "辅助功能启动失败");
            }
        })
    {
        tracing::error!(%error, "无法启动辅助功能线程");
    }
}

fn run() -> windows::core::Result<()> {
    unsafe {
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    {
        let class = w!("QingjianAccessibilityWindow");
        unsafe {
            RegisterClassExW(&WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wndproc),
                hInstance: module(),
                lpszClassName: class,
                ..Default::default()
            });
        }
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                w!(""),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(module()),
                None,
            )?
        };
        let (sender, receiver) = mpsc::channel();
        let mut controller = Controller::new(hwnd, sender, receiver)?;
        controller.reload();
        CONTROLLER.with(|slot| *slot.borrow_mut() = Some(controller));
        unsafe {
            SetTimer(Some(hwnd), 1, 1000, None);
        }
        let mut message = MSG::default();
        while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        CONTROLLER.with(|slot| slot.borrow_mut().take());
        Ok(())
    }
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if [WM_HOTKEY, WM_TIMER, WAKE].contains(&message) {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            CONTROLLER.with(|slot| {
                if let Ok(mut slot) = slot.try_borrow_mut()
                    && let Some(controller) = slot.as_mut()
                {
                    match message {
                        WM_HOTKEY => controller.start(wparam.0 == 2),
                        WM_TIMER => controller.tick(),
                        _ => controller.events(),
                    }
                }
            })
        }));
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn module() -> HINSTANCE {
    HINSTANCE(unsafe { GetModuleHandleW(None) }.unwrap_or_default().0)
}
