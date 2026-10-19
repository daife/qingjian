//! GDI 位图的所有权与截图裁剪；只在截图工作线程中使用。

use std::ffi::c_void;
use windows::Win32::Foundation::{E_FAIL, RECT};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleDC, CreateDIBSection,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HBITMAP, HDC, HGDIOBJ, ReleaseDC, SRCCOPY,
    SelectObject,
};
use windows::core::{Error, Result};

pub(super) struct Bitmap {
    pub(super) dc: HDC,

    bitmap: HBITMAP,

    previous: HGDIOBJ,

    pixels: *mut u8,

    width: i32,

    height: i32,
}

impl Bitmap {
    fn new(width: i32, height: i32) -> Result<Self> {
        // 防止异常虚拟桌面尺寸分配几 GB 内存。
        if width <= 0 || height <= 0 || i64::from(width) * i64::from(height) > 100_000_000 {
            return Err(Error::from(E_FAIL));
        }
        let dc = unsafe { CreateCompatibleDC(None) };
        if dc.is_invalid() {
            return Err(Error::from_thread());
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels: *mut c_void = std::ptr::null_mut();
        let bitmap = match unsafe {
            CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut pixels, None, 0)
        } {
            Ok(bitmap) => bitmap,
            Err(error) => {
                let _ = unsafe { DeleteDC(dc) };
                return Err(error);
            }
        };
        let previous = unsafe { SelectObject(dc, bitmap.into()) };
        Ok(Self {
            dc,
            bitmap,
            previous,
            pixels: pixels.cast(),
            width,
            height,
        })
    }

    pub(super) fn capture(bounds: RECT) -> Result<Self> {
        let bitmap = Self::new(bounds.right - bounds.left, bounds.bottom - bounds.top)?;
        let screen = unsafe { GetDC(None) };
        let copied = unsafe {
            BitBlt(
                bitmap.dc,
                0,
                0,
                bitmap.width,
                bitmap.height,
                Some(screen),
                bounds.left,
                bounds.top,
                SRCCOPY | CAPTUREBLT,
            )
        };
        unsafe { ReleaseDC(None, screen) };
        copied?;
        // CPU 读取 DIB 前完成 GDI 写入。
        let _ = unsafe { windows::Win32::Graphics::Gdi::GdiFlush() };
        Ok(bitmap)
    }

    pub(super) fn black() -> Result<Self> {
        let bitmap = Self::new(1, 1)?;
        unsafe {
            std::ptr::write_bytes(bitmap.pixels, 0, 4);
        }
        Ok(bitmap)
    }

    pub(super) fn crop(&self, rect: RECT) -> Result<Vec<u8>> {
        if rect.left < 0
            || rect.top < 0
            || rect.right > self.width
            || rect.bottom > self.height
            || rect.right <= rect.left
            || rect.bottom <= rect.top
        {
            return Err(Error::from(E_FAIL));
        }
        let width = (rect.right - rect.left) as usize;
        let height = (rect.bottom - rect.top) as usize;
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, width as u32, height as u32);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_compression(png::Compression::Fast);
            let mut writer = encoder.write_header().map_err(|_| Error::from(E_FAIL))?;
            let mut stream = writer.stream_writer().map_err(|_| Error::from(E_FAIL))?;
            let mut row = vec![0; width * 3];
            for y in rect.top..rect.bottom {
                let offset = (y as usize * self.width as usize + rect.left as usize) * 4;
                let pixels =
                    unsafe { std::slice::from_raw_parts(self.pixels.add(offset), width * 4) };
                for (pixel, rgb) in pixels.chunks_exact(4).zip(row.chunks_exact_mut(3)) {
                    rgb.copy_from_slice(&[pixel[2], pixel[1], pixel[0]]);
                }
                std::io::Write::write_all(&mut stream, &row).map_err(|_| Error::from(E_FAIL))?;
            }
            stream.finish().map_err(|_| Error::from(E_FAIL))?;
        }
        Ok(png)
    }
}

impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_uses_original_pixels_and_correct_top_down_rgb_order() {
        let bitmap = Bitmap::new(3, 2).unwrap();
        let pixels = [
            0, 0, 0, 0, 3, 2, 1, 0, 6, 5, 4, 0, 0, 0, 0, 0, 9, 8, 7, 0, 12, 11, 10, 0,
        ];
        unsafe {
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), bitmap.pixels, pixels.len());
        }
        let png = bitmap
            .crop(RECT {
                left: 1,
                top: 0,
                right: 3,
                bottom: 2,
            })
            .unwrap();
        let mut decoder = png::Decoder::new(std::io::Cursor::new(png))
            .read_info()
            .unwrap();
        let mut decoded = vec![0; decoder.output_buffer_size().unwrap()];
        let info = decoder.next_frame(&mut decoded).unwrap();
        assert_eq!((info.width, info.height), (2, 2));
        assert_eq!(
            &decoded[..info.buffer_size()],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
        );
        assert!(
            bitmap
                .crop(RECT {
                    left: -1,
                    top: 0,
                    right: 2,
                    bottom: 2
                })
                .is_err()
        );
    }
}
