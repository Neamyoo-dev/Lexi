use std::sync::Mutex;
use windows::Win32::Foundation::{BOOL, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, FILE_GENERIC_READ, FILE_GENERIC_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::OVERLAPPED;
use windows::Win32::System::Pipes::WaitNamedPipeW;
use windows::Win32::System::Threading::{
    CreateEventW, WaitForSingleObject,
};

use windows::core::PCWSTR;

const BUFFER_SIZE: usize = 4096;
const PIPE_TIMEOUT_MS: u32 = 500;

static PIPE_HANDLE: Mutex<Option<isize>> = Mutex::new(None);

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
            None,
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            None,
        )
    }
    .map_err(|_| "Failed to open pipe".to_string())?;

    *handle_guard = Some(from_handle(handle));
    Ok(())
}

pub fn disconnect() {
    let mut handle_guard = PIPE_HANDLE.lock().unwrap();
    if let Some(handle) = handle_guard.take() {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(to_handle(handle));
        }
    }
}

pub fn send_message(data: &str) -> Result<Option<String>, String> {
    let handle_guard = PIPE_HANDLE.lock().unwrap();
    let handle = handle_guard.as_ref().ok_or("Pipe not connected")?;
    let h = to_handle(*handle);

    // Write
    let mut write_overlapped = OVERLAPPED::default();
    let write_event = unsafe {
        CreateEventW(None, BOOL(0), BOOL(0), PCWSTR::null())
    }
    .map_err(|_| "Failed to create write event".to_string())?;
    write_overlapped.hEvent = write_event;

    let mut written = 0u32;
    unsafe {
        windows::Win32::Storage::FileSystem::WriteFile(
            h,
            Some(data.as_bytes()),
            Some(&mut written),
            Some(&mut write_overlapped as *mut OVERLAPPED),
        )
    }
    .map_err(|_| "Pipe write failed".to_string())?;

    let wait = unsafe { WaitForSingleObject(write_event, PIPE_TIMEOUT_MS) };
    if wait != WAIT_OBJECT_0 {
        unsafe { let _ = windows::Win32::Foundation::CloseHandle(write_event); }
        return Err("Pipe write timed out".into());
    }
    unsafe { let _ = windows::Win32::Foundation::CloseHandle(write_event); }

    // Read
    let mut read_buffer = vec![0u8; BUFFER_SIZE];
    let mut read_overlapped = OVERLAPPED::default();
    let read_event = unsafe {
        CreateEventW(None, BOOL(0), BOOL(0), PCWSTR::null())
    }
    .map_err(|_| "Failed to create read event".to_string())?;
    read_overlapped.hEvent = read_event;

    let mut read_bytes = 0u32;
    unsafe {
        windows::Win32::Storage::FileSystem::ReadFile(
            h,
            Some(&mut read_buffer),
            Some(&mut read_bytes),
            Some(&mut read_overlapped as *mut OVERLAPPED),
        )
    }
    .map_err(|_| "Pipe read failed".to_string())?;

    let wait = unsafe { WaitForSingleObject(read_event, PIPE_TIMEOUT_MS) };
    if wait == WAIT_OBJECT_0 {
        let mut bytes = 0u32;
        let got = unsafe {
            windows::Win32::System::IO::GetOverlappedResult(
                h, &read_overlapped, &mut bytes, BOOL(0),
            )
        };
        if got.is_ok() {
            read_bytes = bytes;
        }
    }
    unsafe { let _ = windows::Win32::Foundation::CloseHandle(read_event); }

    if read_bytes > 0 {
        let response = String::from_utf8_lossy(&read_buffer[..read_bytes as usize])
            .trim_end_matches('\0')
            .to_string();
        Ok(Some(response))
    } else {
        Ok(None)
    }
}
