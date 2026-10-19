//! 截图选择状态与局部绘制；仅截图线程持有。

use super::bitmap::Bitmap;
use crate::accessibility::control::Control;
use std::sync::Arc;
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_OVER, AlphaBlend, BLENDFUNCTION, BeginPaint, BitBlt, CreateSolidBrush, DeleteObject,
    EndPaint, FrameRect, InvalidateRect, PAINTSTRUCT, SRCCOPY, SetBkMode, SetTextColor,
    TRANSPARENT, TextOutW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GWLP_USERDATA, GetCursorPos, GetWindowRect, SetWindowLongPtrW,
};

pub(super) struct Selection {
    pub(super) original: Bitmap,

    pub(super) black: Bitmap,

    pub(super) bounds: RECT,

    pub(super) windows: Vec<HWND>,

    pub(super) anchor: Option<POINT>,

    pub(super) rect: Option<RECT>,

    pub(super) done: bool,

    pub(super) cancelled: bool,

    pub(super) control: Arc<Control>,
}

impl Selection {
    pub(super) fn close(&mut self) {
        self.done = true;
        if self.windows.is_empty() {
            return;
        }
        // ReleaseCapture / DestroyWindow 可同步重入 wndproc；先解除所有回调中的裸指针。
        for &hwnd in &self.windows {
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
        }
        unsafe {
            let _ = ReleaseCapture();
        }
        for hwnd in self.windows.drain(..) {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }

    pub(super) fn update(&mut self) {
        let Some(anchor) = self.anchor else {
            return;
        };
        let mut point = POINT::default();
        if unsafe { GetCursorPos(&mut point) }.is_err() {
            return;
        }
        let rect = rectangle(anchor, point, self.bounds);
        let old = self.rect.replace(rect).unwrap_or(rect);
        let dirty = RECT {
            left: old.left.min(rect.left) - 3,
            top: old.top.min(rect.top) - 3,
            right: old.right.max(rect.right) + 3,
            bottom: old.bottom.max(rect.bottom) + 3,
        };
        for &hwnd in &self.windows {
            let mut screen = RECT::default();
            if unsafe { GetWindowRect(hwnd, &mut screen) }.is_ok() {
                let local = RECT {
                    left: dirty.left - screen.left,
                    top: dirty.top - screen.top,
                    right: dirty.right - screen.left,
                    bottom: dirty.bottom - screen.top,
                };
                let _ = unsafe { InvalidateRect(Some(hwnd), Some(&local), false) };
                let hint = RECT {
                    left: 0,
                    top: 0,
                    right: 650,
                    bottom: 48,
                };
                let _ = unsafe { InvalidateRect(Some(hwnd), Some(&hint), false) };
            }
        }
    }

    pub(super) fn paint(&self, hwnd: HWND) {
        let mut paint = PAINTSTRUCT::default();
        let dc = unsafe { BeginPaint(hwnd, &mut paint) };
        let mut screen = RECT::default();
        let _ = unsafe { GetWindowRect(hwnd, &mut screen) };
        unsafe {
            let _ = BitBlt(
                dc,
                0,
                0,
                screen.right - screen.left,
                screen.bottom - screen.top,
                Some(self.original.dc),
                screen.left - self.bounds.left,
                screen.top - self.bounds.top,
                SRCCOPY,
            );
            // 一张原图加 1×1 黑色源图，避免额外保存一整张暗色桌面。
            let _ = AlphaBlend(
                dc,
                0,
                0,
                screen.right - screen.left,
                screen.bottom - screen.top,
                self.black.dc,
                0,
                0,
                1,
                1,
                BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 128,
                    AlphaFormat: 0,
                },
            );
            if let Some(rect) = self.rect {
                let local = RECT {
                    left: rect.left - screen.left,
                    top: rect.top - screen.top,
                    right: rect.right - screen.left,
                    bottom: rect.bottom - screen.top,
                };
                let _ = BitBlt(
                    dc,
                    local.left,
                    local.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    Some(self.original.dc),
                    rect.left - self.bounds.left,
                    rect.top - self.bounds.top,
                    SRCCOPY,
                );
                let brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00E9B431));
                FrameRect(dc, &local, brush);
                let _ = DeleteObject(brush.into());
            }
            SetBkMode(dc, TRANSPARENT);
            SetTextColor(dc, windows::Win32::Foundation::COLORREF(0x00FFFFFF));
            let size = self
                .rect
                .map(|rect| format!("  {} × {}", rect.right - rect.left, rect.bottom - rect.top))
                .unwrap_or_default();
            let hint: Vec<u16> = format!("拖动框选文字 · Esc / 右键取消{size}")
                .encode_utf16()
                .collect();
            let _ = TextOutW(dc, 20, 18, &hint);
            let _ = EndPaint(hwnd, &paint);
        }
    }
}

impl Drop for Selection {
    fn drop(&mut self) {
        self.close();
    }
}

fn rectangle(a: POINT, b: POINT, bounds: RECT) -> RECT {
    RECT {
        left: a.x.min(b.x).clamp(bounds.left, bounds.right),
        top: a.y.min(b.y).clamp(bounds.top, bounds.bottom),
        right: a.x.max(b.x).clamp(bounds.left, bounds.right),
        bottom: a.y.max(b.y).clamp(bounds.top, bounds.bottom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reverse_drag_crosses_negative_monitor_coordinates() {
        let bounds = RECT {
            left: -1920,
            top: -200,
            right: 2560,
            bottom: 1440,
        };
        let rect = rectangle(
            POINT { x: 500, y: 600 },
            POINT { x: -1500, y: -400 },
            bounds,
        );
        assert_eq!(
            (rect.left, rect.top, rect.right, rect.bottom),
            (-1500, -200, 500, 600)
        );
    }
}
