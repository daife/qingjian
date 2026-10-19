//! 每屏一个不透明置顶窗口；选区只是绘制原图，不创建会穿透鼠标的透明洞。

mod bitmap;
mod selection;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::SetCapture;
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CreateWindowExW, DefWindowProcW, DispatchMessageW, GWLP_USERDATA, GetCursorPos,
    GetForegroundWindow, GetMessageW, GetSystemMetrics, GetWindowLongPtrW, HWND_TOPMOST, IDC_CROSS,
    LoadCursorW, MSG, RegisterClassExW, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN, SW_SHOW, SWP_NOACTIVATE, SetForegroundWindow, SetTimer, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, TranslateMessage, WM_ACTIVATEAPP, WM_CANCELMODE, WM_CAPTURECHANGED,
    WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE, WM_PAINT,
    WM_RBUTTONDOWN, WM_TIMER, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{BOOL, w};

use self::bitmap::Bitmap;
use self::selection::Selection;
use super::control::Control;

pub(super) fn select(control: Arc<Control>) -> Result<Option<Vec<u8>>, String> {
    unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    select_inner(control).map_err(|_| "无法截取桌面，请退出安全桌面或检查显示器状态".to_owned())
}

fn select_inner(control: Arc<Control>) -> windows::core::Result<Option<Vec<u8>>> {
    let original_foreground = unsafe { GetForegroundWindow() };
    let bounds = unsafe {
        RECT {
            left: GetSystemMetrics(SM_XVIRTUALSCREEN),
            top: GetSystemMetrics(SM_YVIRTUALSCREEN),
            right: GetSystemMetrics(SM_XVIRTUALSCREEN) + GetSystemMetrics(SM_CXVIRTUALSCREEN),
            bottom: GetSystemMetrics(SM_YVIRTUALSCREEN) + GetSystemMetrics(SM_CYVIRTUALSCREEN),
        }
    };
    let original = Bitmap::capture(bounds)?;
    let black = Bitmap::black()?;
    let mut selection = Box::new(Selection {
        original,
        black,
        bounds,
        windows: Vec::new(),
        anchor: None,
        rect: None,
        done: false,
        cancelled: false,
        control,
    });
    let class = w!("QingjianScreenshotOverlay");
    unsafe {
        RegisterClassExW(&WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: super::module(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_CROSS)?,
            ..Default::default()
        });
    }
    let mut monitors = Vec::<RECT>::new();
    let _ = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(monitor),
            LPARAM((&mut monitors as *mut Vec<RECT>) as isize),
        )
    };
    for rect in monitors {
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                class,
                w!("青简截图 OCR"),
                WS_POPUP,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                None,
                None,
                Some(super::module()),
                Some((&mut *selection as *mut Selection).cast()),
            )
        };
        let hwnd = match hwnd {
            Ok(hwnd) => hwnd,
            Err(error) => {
                selection.close();
                return Err(error);
            }
        };
        selection.windows.push(hwnd);
    }
    if selection.windows.is_empty() {
        return Err(windows::core::Error::from_thread());
    }
    for &hwnd in &selection.windows {
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE
                    | windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                    | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE,
            );
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
    }
    unsafe {
        let _ = SetForegroundWindow(selection.windows[0]);
        SetTimer(Some(selection.windows[0]), 1, 100, None);
    }
    let mut message = MSG::default();
    while !selection.done && unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    let restore = selection
        .windows
        .contains(&unsafe { GetForegroundWindow() });
    selection.close();
    if restore {
        let _ = unsafe { SetForegroundWindow(original_foreground) };
    }
    if selection.cancelled || selection.control.is_cancelled() {
        return Ok(None);
    }
    match selection.rect {
        Some(mut rect) if rect.right > rect.left && rect.bottom > rect.top => {
            rect.left -= bounds.left;
            rect.right -= bounds.left;
            rect.top -= bounds.top;
            rect.bottom -= bounds.top;
            selection.original.crop(rect).map(Some)
        }
        _ => Ok(None),
    }
}

unsafe extern "system" fn monitor(handle: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
    let monitors = unsafe { &mut *(data.0 as *mut Vec<RECT>) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(handle, &mut info) }.as_bool() {
        monitors.push(info.rcMonitor);
    }
    true.into()
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let result = catch_unwind(AssertUnwindSafe(|| unsafe {
        process(hwnd, message, wparam, lparam)
    }));
    result.unwrap_or(LRESULT(0))
}

unsafe fn process(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
    }
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Selection;
    if pointer.is_null() {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    let selection = unsafe { &mut *pointer };
    match message {
        WM_ERASEBKGND => return LRESULT(1),
        WM_PAINT => {
            selection.paint(hwnd);
            return LRESULT(0);
        }
        WM_LBUTTONDOWN => {
            let mut point = POINT::default();
            let _ = unsafe { GetCursorPos(&mut point) };
            selection.anchor = Some(point);
            unsafe {
                SetCapture(hwnd);
            }
            selection.update();
        }
        WM_MOUSEMOVE => selection.update(),
        WM_LBUTTONUP if selection.anchor.is_some() => {
            selection.update();
            selection.done = true;
        }
        WM_RBUTTONDOWN | WM_CANCELMODE => {
            selection.done = true;
            selection.cancelled = true;
        }
        WM_KEYDOWN if wparam.0 == 0x1B => {
            selection.done = true;
            selection.cancelled = true;
        }
        WM_CAPTURECHANGED if selection.anchor.is_some() && !selection.done => {
            selection.done = true;
            selection.cancelled = true;
        }
        WM_ACTIVATEAPP if wparam.0 == 0 && !selection.done => {
            selection.done = true;
            selection.cancelled = true;
        }
        WM_TIMER if selection.control.is_cancelled() => {
            selection.done = true;
            selection.cancelled = true;
        }
        _ => return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
    LRESULT(0)
}
