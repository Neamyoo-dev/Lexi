use std::sync::Mutex;
use windows::core::HRESULT;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::OVERLAPPED;
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

const PIPE_NAME: &str = r"\\.\pipe\LexiInputMethod";
const PIPE_TIMEOUT_MS: u32 = 5000;

struct PipeHandle(HANDLE);
unsafe impl Send for PipeHandle {}
unsafe impl Sync for PipeHandle {}

static PIPE_HANDLE: Mutex<Option<PipeHandle>> = Mutex::new(None);

pub fn connect() -> Result<HANDLE, HRESULT> {
    let mut handle = PIPE_HANDLE.lock().map_err(|_| HRESULT(-1))?;

    if let Some(ref h) = *handle {
        return Ok(h.0);
    }

    let pipe_path = encode_wide(PIPE_NAME);

    unsafe {
        let h = CreateFileW(
            pcwstr(&pipe_path),
            windows::Win32::Storage::FileSystem::GENERIC_READ
                | windows::Win32::Storage::FileSystem::GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            None,
        );

        if h.is_invalid() {
            return Err(HRESULT(-1));
        }

        *handle = Some(PipeHandle(h));
        Ok(h)
    }
}

pub fn disconnect() {
    if let Ok(mut handle) = PIPE_HANDLE.lock() {
        if let Some(h) = handle.take() {
            unsafe {
                CloseHandle(h.0);
            }
        }
    }
}

pub fn send_message(data: &str) -> Result<Option<String>, HRESULT> {
    let h = match connect() {
        Ok(h) => h,
        Err(_) => return Ok(None),
    };

    let request = data.as_bytes();
    let mut response = vec![0u8; 4096];

    let write_event = unsafe { CreateEventW(None, true, false, None) };
    if write_event.is_invalid() {
        return Err(HRESULT(-1));
    }

    let read_event = unsafe { CreateEventW(None, true, false, None) };
    if read_event.is_invalid() {
        unsafe {
            CloseHandle(write_event);
        }
        return Err(HRESULT(-1));
    }

    let mut write_overlapped = create_overlapped(write_event);
    let write_ok = unsafe {
        windows::Win32::Storage::FileSystem::WriteFile(
            h,
            Some(request),
            Some(&mut write_overlapped),
        )
        .is_ok()
    };

    if !write_ok {
        unsafe {
            CloseHandle(write_event);
            CloseHandle(read_event);
        }
        return Err(HRESULT(-1));
    }

    let wait_result = unsafe { WaitForSingleObject(write_event, PIPE_TIMEOUT_MS) };
    unsafe {
        CloseHandle(write_event);
    }

    if wait_result.0 != 0 {
        unsafe {
            CloseHandle(read_event);
        }
        return Err(HRESULT(-1));
    }

    let mut read_overlapped = create_overlapped(read_event);
    let read_ok = unsafe {
        windows::Win32::Storage::FileSystem::ReadFile(
            h,
            Some(&mut response),
            Some(&mut read_overlapped),
        )
        .is_ok()
    };

    if !read_ok {
        unsafe {
            CloseHandle(read_event);
        }
        return Ok(None);
    }

    let wait_result = unsafe { WaitForSingleObject(read_event, PIPE_TIMEOUT_MS) };
    let mut bytes_read: u32 = 0;
    if wait_result.0 == 0 {
        unsafe {
            let _ = windows::Win32::System::IO::GetOverlappedResult(
                h,
                &read_overlapped,
                &mut bytes_read,
                false,
            );
        }
    }
    unsafe {
        CloseHandle(read_event);
    }

    if bytes_read > 0 {
        let response_str = String::from_utf8_lossy(&response[..bytes_read as usize]);
        let trimmed = response_str.trim_end_matches('\0').trim().to_string();
        if trimmed.is_empty() {
            Ok(None)
        } else {
            Ok(Some(trimmed))
        }
    } else {
        Ok(None)
    }
}

fn create_overlapped(event: HANDLE) -> OVERLAPPED {
    OVERLAPPED {
        Internal: 0,
        InternalHigh: 0,
        Anonymous: windows::Win32::System::IO::OVERLAPPED_0 {
            Anonymous: windows::Win32::System::IO::OVERLAPPED_0_0 {
                Offset: 0,
                OffsetHigh: 0,
            },
        },
        hEvent: event,
    }
}

fn encode_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn pcwstr(vec: &[u16]) -> windows::core::PCWSTR {
    windows::core::PCWSTR(vec.as_ptr())
}
