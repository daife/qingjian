//! 保存原编辑目标；结果只发回同一目标，TSF 一旦受理就不再补发粘贴。

use qingjian_platform::protocol::{ASSIST_CAPTURE_MESSAGE, ASSIST_COPY_TAG};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::DataExchange::{
    COPYDATASTRUCT, CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::UI::Accessibility::{
    CUIAutomation8, IUIAutomation, IUIAutomation2, IUIAutomationElement, IUIAutomationTextPattern,
    IUIAutomationValuePattern, UIA_TextPatternId, UIA_ValuePatternId,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, GUITHREADINFO, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId,
    HWND_MESSAGE, SMTO_ABORTIFHUNG, SMTO_BLOCK, SendMessageTimeoutW, WM_COPYDATA,
};
use windows::core::{Interface, w};

pub(super) struct Target {
    foreground: HWND,

    focus: HWND,

    automation: Option<IUIAutomation>,

    element: Option<IUIAutomationElement>,

    tsf: Option<HWND>,

    token: u32,
}

impl Target {
    pub(super) fn seed() -> (isize, isize) {
        let foreground = unsafe { GetForegroundWindow() };
        let thread = unsafe { GetWindowThreadProcessId(foreground, None) };
        let mut gui = GUITHREADINFO {
            cbSize: size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        let _ = unsafe { GetGUIThreadInfo(thread, &mut gui) };
        (foreground.0 as isize, gui.hwndFocus.0 as isize)
    }

    pub(super) fn capture(token: u32, seed: (isize, isize)) -> Self {
        let foreground = HWND(seed.0 as *mut _);
        let thread = unsafe { GetWindowThreadProcessId(foreground, None) };
        let mut gui = GUITHREADINFO {
            cbSize: size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        let _ = unsafe { GetGUIThreadInfo(thread, &mut gui) };
        let automation: Option<IUIAutomation> =
            unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) }.ok();
        if let Some(uia) = &automation
            && let Ok(options) = uia.cast::<IUIAutomation2>()
        {
            unsafe {
                let _ = options.SetAutoSetFocus(false);
                let _ = options.SetConnectionTimeout(300);
                let _ = options.SetTransactionTimeout(300);
            }
        }
        let element = automation
            .as_ref()
            .and_then(|uia| unsafe { uia.GetFocusedElement() }.ok());
        let tsf = find_tsf(thread).filter(|hwnd| {
            let mut result = 0;
            let sent = unsafe {
                SendMessageTimeoutW(
                    *hwnd,
                    ASSIST_CAPTURE_MESSAGE,
                    WPARAM(token as usize),
                    LPARAM(0),
                    SMTO_ABORTIFHUNG | SMTO_BLOCK,
                    150,
                    Some(&mut result),
                )
            };
            sent.0 != 0 && result == 1
        });
        Self {
            foreground,
            focus: HWND(seed.1 as *mut _),
            automation,
            element,
            tsf,
            token,
        }
    }

    fn unchanged(&self) -> bool {
        if unsafe { GetForegroundWindow() } != self.foreground {
            return false;
        }
        let thread = unsafe { GetWindowThreadProcessId(self.foreground, None) };
        let mut gui = GUITHREADINFO {
            cbSize: size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetGUIThreadInfo(thread, &mut gui) }.is_err() || gui.hwndFocus != self.focus {
            return false;
        }
        match (&self.automation, &self.element) {
            (Some(uia), Some(element)) => unsafe {
                uia.GetFocusedElement()
                    .and_then(|current| uia.CompareElements(element, &current))
            }
            .is_ok_and(|same| same.as_bool()),
            _ => self.tsf.is_some(),
        }
    }

    pub(super) fn insert(&self, owner: HWND, text: &str) -> &'static str {
        if !self.unchanged() {
            return "已复制；输入目标已改变，请手动粘贴";
        }
        if [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN]
            .iter()
            .any(|key| unsafe { GetAsyncKeyState(i32::from(key.0)) } < 0)
        {
            return "已复制；请松开修饰键后手动粘贴";
        }
        if let Some(hwnd) = self.tsf {
            let mut data = self.token.to_le_bytes().to_vec();
            for unit in text.encode_utf16() {
                data.extend_from_slice(&unit.to_le_bytes());
            }
            let copy = COPYDATASTRUCT {
                dwData: ASSIST_COPY_TAG,
                cbData: data.len() as u32,
                lpData: data.as_mut_ptr().cast(),
            };
            let mut result = 0;
            let sent = unsafe {
                SendMessageTimeoutW(
                    hwnd,
                    WM_COPYDATA,
                    WPARAM(owner.0 as usize),
                    LPARAM((&copy as *const COPYDATASTRUCT) as isize),
                    SMTO_ABORTIFHUNG | SMTO_BLOCK,
                    200,
                    Some(&mut result),
                )
            };
            // 超时也可能已经排入异步编辑会话，不能再粘贴第二次。
            return if sent.0 != 0 && result == 1 {
                "已复制，并已请求写入原输入框"
            } else {
                "已复制；未能确认输入框写入，请检查后手动粘贴"
            };
        }
        let editable = self.element.as_ref().is_some_and(editable);
        if !editable || !self.unchanged() {
            return "已复制；当前控件无法自动写入，请手动粘贴";
        }
        let inputs = [
            key(VK_CONTROL, false),
            key(VIRTUAL_KEY(0x56), false),
            key(VIRTUAL_KEY(0x56), true),
            key(VK_CONTROL, true),
        ];
        if unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) } == inputs.len() as u32 {
            "已复制，并已向原输入框粘贴"
        } else {
            "已复制；系统限制了自动写入，请手动粘贴"
        }
    }
}

impl Drop for Target {
    fn drop(&mut self) {
        if let Some(hwnd) = self.tsf {
            let _ = unsafe {
                SendMessageTimeoutW(
                    hwnd,
                    ASSIST_CAPTURE_MESSAGE,
                    WPARAM(self.token as usize),
                    LPARAM(1),
                    SMTO_ABORTIFHUNG | SMTO_BLOCK,
                    100,
                    None,
                )
            };
        }
    }
}

fn editable(element: &IUIAutomationElement) -> bool {
    unsafe {
        if element
            .CurrentIsPassword()
            .map_or(true, |password| password.as_bool())
        {
            return false;
        }
        if !element
            .CurrentIsEnabled()
            .is_ok_and(|enabled| enabled.as_bool())
        {
            return false;
        }
        if let Ok(value) =
            element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
        {
            return value
                .CurrentIsReadOnly()
                .is_ok_and(|readonly| !readonly.as_bool());
        }
        // 富文本常不提供 ValuePattern；TextPattern 的只读属性要从文档范围取得。
        element
            .GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
            .and_then(|pattern| pattern.DocumentRange())
            .and_then(|range| {
                range
                    .GetAttributeValue(windows::Win32::UI::Accessibility::UIA_IsReadOnlyAttributeId)
            })
            .ok()
            .and_then(|value| bool::try_from(&value).ok())
            .is_some_and(|readonly| !readonly)
    }
}

fn find_tsf(thread: u32) -> Option<HWND> {
    let mut after = None;
    while let Ok(hwnd) =
        unsafe { FindWindowExW(Some(HWND_MESSAGE), after, w!("QingjianPollWindow"), None) }
    {
        if unsafe { GetWindowThreadProcessId(hwnd, None) } == thread {
            return Some(hwnd);
        }
        after = Some(hwnd);
    }
    None
}

fn key(code: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: code,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                },
                ..Default::default()
            },
        },
    }
}

pub(super) fn clipboard(owner: HWND, text: &str) -> windows::core::Result<()> {
    let bytes: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    let memory = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len() * 2)? };
    let pointer = unsafe { GlobalLock(memory) };
    if pointer.is_null() {
        let _ = unsafe { GlobalFree(Some(memory)) };
        return Err(windows::core::Error::from_thread());
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer.cast(), bytes.len());
        let _ = GlobalUnlock(memory);
    }
    let result = (|| unsafe {
        OpenClipboard(Some(owner))?;
        let result = EmptyClipboard()
            .and_then(|()| SetClipboardData(13, Some(HANDLE(memory.0))).map(|_| ()));
        let _ = CloseClipboard();
        result
    })();
    if result.is_err() {
        let _ = unsafe { GlobalFree(Some(HGLOBAL(memory.0))) };
    }
    result
}
