use std::sync::Mutex;
use windows::Win32::Foundation::{BOOL, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, FILE_GENERIC_READ, FILE_GENERIC_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::OVERLAPPED;
use windows::Win32::System::Pipes::WaitNamedPipeW;
use windows::Win32::System::Threading::{
    CreateEventW, WaitForSingleObject,
};

use windows::core::PCWSTR;

const PIPE_NAME: &str = r"\\.\pipe\LexiInputMethod";
const BUFFER_SIZE: usize = 4096;
const PIPE_TIMEOUT_MS: u32 = 500;

static PIPE_HANDLE: Mutex<Option<isize>> = Mutex::new(None);
static PIPE_STATE: Mutex<Option<PipeState>> = Mutex::new(None);

struct SafeOverlapped(OVERLAPPED);
unsafe impl Send for SafeOverlapped {}
unsafe impl Sync for SafeOverlapped {}

struct PipeState {
    handle: isize,
    overlapped: SafeOverlapped,
}

fn to_handle(val: isize) -> HANDLE {
    HANDLE(val as *mut std::ffi::c_void)
}

fn from_handle(handle: HANDLE) -> isize {
    handle.0 as isize
}

pub fn connect() -> Result<(), String> {
    let mut handle_guard = PIPE_HANDLE.lock().unwrap();
    if handle_guard.is_some() {
        return Ok(());
    }

    unsafe {
        let _ = WaitNamedPipeW(
            windows::core::w!("\\\\.\\pipe\\LexiInputMethod"),
            PIPE_TIMEOUT_MS,
        );
    }

    let handle = unsafe {
        CreateFileW(
            windows::core::w!("\\\\.\\pipe\\LexiInputMethod"),
            FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
            windows::Win32::Storage::FileSystem::FILE_SHARE_READ
                | windows::Win32::Storage::FileSystem::FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            None,
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        return Err("Failed to open pipe".into());
    }

    let event = unsafe {
        CreateEventW(
            std::ptr::null_mut(),
            BOOL(0),
            BOOL(0),
            PCWSTR::null(),
        )
    };

    if event.is_invalid() {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(handle);
        }
        return Err("Failed to create event".into());
    }

    let mut overlapped = OVERLAPPED::default();
    overlapped.hEvent = event;

    let handle_isize = from_handle(handle);
    let state = PipeState { handle: handle_isize, overlapped: SafeOverlapped(overlapped) };
    *handle_guard = Some(handle_isize);
    *PIPE_STATE.lock().unwrap() = Some(state);

    Ok(())
}

pub fn disconnect() {
    let mut handle_guard = PIPE_HANDLE.lock().unwrap();
    if let Some(handle) = handle_guard.take() {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(to_handle(handle));
        }
    }
    *PIPE_STATE.lock().unwrap() = None;
}

pub fn send_message(data: &str) -> Result<Option<String>, String> {
    let handle_guard = PIPE_HANDLE.lock().unwrap();

    if let Some(handle) = handle_guard.as_ref() {
        let h = to_handle(*handle);
        let send_bytes = data.as_bytes();
        let mut send_buffer = send_bytes.to_vec();

        let mut write_overlapped = OVERLAPPED::default();
        let write_event = unsafe {
            CreateEventW(std::ptr::null_mut(), BOOL(0), BOOL(0), PCWSTR::null())
        };
        if write_event.is_invalid() {
            return Err("Failed to create write event".into());
        }
        write_overlapped.hEvent = write_event;

        let mut written = 0u32;
        let write_result = unsafe {
            windows::Win32::Storage::FileSystem::WriteFile(
                h,
                Some(send_buffer.as_ref()),
                Some(&mut written),
                Some(&write_overlapped),
            )
        };

        let write_ok = if !write_result.as_bool() {
            let err = unsafe { windows::Win32::Foundation::GetLastError() };
            if err.0 == 997 {
                let wait = unsafe {
                    WaitForSingleObject(write_event, PIPE_TIMEOUT_MS)
                };
                if wait == WAIT_OBJECT_0 {
                    let mut bytes = 0u32;
                    let got = unsafe {
                        windows::Win32::System::IO::GetOverlappedResult(h, &write_overlapped, &mut bytes, BOOL(0))
                    };
                    got.as_bool()
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            true
        };

        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(write_event);
        }

        if !write_ok {
            return Err("Pipe write failed or timed out".into());
        }

        let mut read_buffer = vec![0u8; BUFFER_SIZE];
        let mut read_overlapped = OVERLAPPED::default();
        let read_event = unsafe {
            CreateEventW(std::ptr::null_mut(), BOOL(0), BOOL(0), PCWSTR::null())
        };
        if read_event.is_invalid() {
            return Err("Failed to create read event".into());
        }
        read_overlapped.hEvent = read_event;

        let mut read_bytes = 0u32;
        let read_result = unsafe {
            windows::Win32::Storage::FileSystem::ReadFile(
                h,
                Some(&mut read_buffer),
                Some(&mut read_bytes),
                Some(&read_overlapped),
            )
        };

        let read_ok = if !read_result.as_bool() {
            let err = unsafe { windows::Win32::Foundation::GetLastError() };
            if err.0 == 997 {
                let wait = unsafe {
                    WaitForSingleObject(read_event, PIPE_TIMEOUT_MS)
                };
                if wait == WAIT_OBJECT_0 {
                    let mut bytes = 0u32;
                    let got = unsafe {
                        windows::Win32::System::IO::GetOverlappedResult(h, &read_overlapped, &mut bytes, BOOL(0))
                    };
                    read_bytes = bytes;
                    got.as_bool()
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            true
        };

        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(read_event);
        }

        if read_ok && read_bytes > 0 {
            let response = String::from_utf8_lossy(&read_buffer[..read_bytes as usize])
                .trim_end_matches('\0')
                .to_string();
            Ok(Some(response))
        } else {
            Ok(None)
        }
    } else {
        Err("Pipe not connected".into())
    }
}
