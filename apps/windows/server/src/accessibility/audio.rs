//! WASAPI 共享模式事件驱动录音；系统音频引擎负责降混与采样率转换。

use super::control::Control;
use qingjian_recognize::{AUDIO_RATE, MAX_AUDIO_SECONDS};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
    IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator, WAVEFORMATEX,
    eCapture, eConsole,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::Win32::System::Threading::{CreateEventW, WaitForMultipleObjects};

pub(super) fn record(control: &Control, ready: impl FnOnce()) -> Result<Option<Vec<u8>>, String> {
    let _apartment = super::apartment::Apartment::new().map_err(|_| "无法初始化录音".to_owned())?;
    let result = unsafe { capture(control, ready) };
    result.map_err(|_| "无法录音，请检查默认麦克风、系统麦克风权限或设备是否已断开".to_owned())
}

unsafe fn capture(
    control: &Control,
    ready: impl FnOnce(),
) -> windows::core::Result<Option<Vec<u8>>> {
    let devices: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let device = unsafe { devices.GetDefaultAudioEndpoint(eCapture, eConsole)? };
    let client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None)? };
    let format = WAVEFORMATEX {
        wFormatTag: 1,
        nChannels: 1,
        nSamplesPerSec: AUDIO_RATE,
        nAvgBytesPerSec: AUDIO_RATE * 2,
        nBlockAlign: 2,
        wBitsPerSample: 16,
        cbSize: 0,
    };
    unsafe {
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK
                | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            1_000_000,
            0,
            &format,
            None,
        )?
    };
    let event = unsafe { CreateEventW(None, false, false, None)? };
    let result = (|| {
        unsafe { client.SetEventHandle(event)? };
        let capture: IAudioCaptureClient = unsafe { client.GetService()? };
        unsafe { client.Start()? };
        ready();
        let start = Instant::now();
        let maximum = (AUDIO_RATE * MAX_AUDIO_SECONDS * 2) as usize;
        let mut pcm = Vec::with_capacity(maximum);
        let result = (|| {
            while control.is_running() && pcm.len() < maximum {
                let remaining = Duration::from_secs(u64::from(MAX_AUDIO_SECONDS))
                    .saturating_sub(start.elapsed());
                if remaining.is_zero() {
                    break;
                }
                let wait = unsafe {
                    WaitForMultipleObjects(
                        &[control.event(), event],
                        false,
                        remaining.as_millis().min(u128::from(u32::MAX)) as u32,
                    )
                };
                if wait == WAIT_OBJECT_0 || wait == WAIT_TIMEOUT {
                    break;
                }
                if wait.0 != WAIT_OBJECT_0.0 + 1 {
                    return Err(windows::core::Error::from_thread());
                }
                drain(&capture, &mut pcm, maximum)?;
            }
            if !control.is_cancelled() {
                drain(&capture, &mut pcm, maximum)?;
            }
            Ok(())
        })();
        let _ = unsafe { client.Stop() };
        result?;
        if control.is_cancelled() {
            return Ok(None);
        }
        Ok(Some(wav(&pcm)))
    })();
    let _ = unsafe { CloseHandle(event) };
    result
}

fn drain(
    capture: &IAudioCaptureClient,
    pcm: &mut Vec<u8>,
    maximum: usize,
) -> windows::core::Result<()> {
    while unsafe { capture.GetNextPacketSize()? } > 0 {
        let (mut data, mut frames, mut flags) = (std::ptr::null_mut(), 0, 0);
        unsafe { capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None)? };
        let bytes = (frames as usize * 2).min(maximum.saturating_sub(pcm.len()));
        if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
            pcm.resize(pcm.len() + bytes, 0);
        } else if !data.is_null() {
            pcm.extend_from_slice(unsafe { std::slice::from_raw_parts(data, bytes) });
        }
        unsafe { capture.ReleaseBuffer(frames)? };
    }
    Ok(())
}

fn wav(pcm: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(44 + pcm.len());
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&AUDIO_RATE.to_le_bytes());
    bytes.extend_from_slice(&(AUDIO_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    bytes.extend_from_slice(pcm);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sixty_seconds_have_exact_pcm_header_and_size() {
        let bytes = wav(&vec![0; (AUDIO_RATE * MAX_AUDIO_SECONDS * 2) as usize]);
        assert_eq!(bytes.len(), 1_920_044);
        assert_eq!(
            u32::from_le_bytes(bytes[40..44].try_into().unwrap()),
            1_920_000
        );
        assert_eq!(&bytes[24..28], &16_000u32.to_le_bytes());
    }
}
