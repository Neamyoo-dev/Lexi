mod pipe_client;
mod text_service;

use std::sync::atomic::{AtomicBool, Ordering};
use windows::core::{implement, GUID, HRESULT, IUnknown, PCWSTR};
use windows::Win32::Foundation::{BOOL, HINSTANCE};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CLASSES_ROOT,
    HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS, REG_SZ,
};
use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};

pub const CLSID_LEXI_IME: GUID = GUID::from_u128(0x12340001_0000_0000_C000_000000000046);
pub const GUID_LEXI_PROFILE: GUID = GUID::from_u128(0x12340002_0000_0000_C000_000000000046);

struct SafeHInstance(isize);

unsafe impl Send for SafeHInstance {}
unsafe impl Sync for SafeHInstance {}

static DLL_INSTANCE: std::sync::OnceLock<SafeHInstance> = std::sync::OnceLock::new();
static LEXI_ACTIVE: AtomicBool = AtomicBool::new(false);

fn get_dll_path() -> Option<String> {
    let instance = DLL_INSTANCE.get()?;
    let handle = HINSTANCE(instance.0 as *mut std::ffi::c_void);
    let mut buf = vec![0u16; 1024];
    let len = unsafe {
        GetModuleFileNameW(
            Some(handle),
            &mut buf,
        )
    };
    if len == 0 {
        return None;
    }
    buf.truncate(len as usize);
    Some(String::from_utf16_lossy(&buf))
}

fn clsid_to_string(clsid: &GUID) -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        clsid.data1,
        clsid.data2,
        clsid.data3,
        clsid.data4[0],
        clsid.data4[1],
        clsid.data4[2],
        clsid.data4[3],
        clsid.data4[4],
        clsid.data4[5],
        clsid.data4[6],
        clsid.data4[7],
    )
}

fn set_reg_value(hkey: HKEY, sub_key: &str, value_name: &str, value: &str) -> bool {
    let sub_key_u16: Vec<u16> = sub_key.encode_utf16().chain(std::iter::once(0)).collect();
    let value_name_u16: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
    let value_u16: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let mut hkey_result: HKEY = HKEY::default();
        let hr = RegCreateKeyExW(
            hkey,
            PCWSTR::from_raw(sub_key_u16.as_ptr()),
            0,
            None,
            0,
            KEY_ALL_ACCESS,
            None,
            &mut hkey_result,
            None,
        );

        if hr.is_ok() {
            let result = RegSetValueExW(
                hkey_result,
                PCWSTR::from_raw(value_name_u16.as_ptr()),
                0,
                REG_SZ,
                Some(&value_u16.iter().map(|&c| c as u8).collect::<Vec<_>>()),
            );
            let _ = RegCloseKey(hkey_result);
            result.is_ok()
        } else {
            false
        }
    }
}

#[no_mangle]
pub extern "system" fn DllMain(
    hinst: HINSTANCE,
    reason: u32,
    _reserved: *mut std::ffi::c_void,
) -> BOOL {
    match reason {
        DLL_PROCESS_ATTACH => {
            DLL_INSTANCE.set(SafeHInstance(hinst.0 as isize)).ok();
        }
        DLL_PROCESS_DETACH => {
            pipe_client::disconnect();
        }
        _ => {}
    }
    BOOL(1)
}

#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut std::ffi::c_void,
) -> HRESULT {
    if rclsid.is_null() || riid.is_null() || ppv.is_null() {
        return HRESULT(-0x7ff8ffffi32);
    }

    let clsid = *rclsid;
    if clsid != CLSID_LEXI_IME {
        return HRESULT(-0x7ff8fffei32);
    }

    let factory = LexiClassFactory::new();
    let factory_unknown: IUnknown = factory.into();

    let result = factory_unknown.query(riid, ppv as *mut *mut std::ffi::c_void);

    if result.is_ok() {
        std::mem::forget(factory_unknown);
    }

    result
}

#[no_mangle]
pub unsafe extern "system" fn DllCanUnloadNow() -> HRESULT {
    if LEXI_ACTIVE.load(Ordering::SeqCst) {
        HRESULT(1)
    } else {
        HRESULT(0)
    }
}

#[no_mangle]
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    register_ime()
}

#[no_mangle]
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    unregister_ime()
}

fn register_ime() -> HRESULT {
    let dll_path = match get_dll_path() {
        Some(p) => p,
        None => return HRESULT(-0x7ff8ffffi32),
    };

    let clsid_str = clsid_to_string(&CLSID_LEXI_IME);

    let com_key = format!("SOFTWARE\\Classes\\CLSID\\{}", clsid_str);
    let inproc_key = format!("SOFTWARE\\Classes\\CLSID\\{}\\InprocServer32", clsid_str);

    if !set_reg_value(
        HKEY_LOCAL_MACHINE,
        &com_key,
        "",
        "Lexi Input Method",
    ) {
        return HRESULT(-0x7ff8ffffi32);
    }

    if !set_reg_value(
        HKEY_LOCAL_MACHINE,
        &inproc_key,
        "",
        &dll_path,
    ) {
        return HRESULT(-0x7ff8ffffi32);
    }

    if !set_reg_value(
        HKEY_LOCAL_MACHINE,
        &inproc_key,
        "ThreadingModel",
        "Apartment",
    ) {
        return HRESULT(-0x7ff8ffffi32);
    }

    let profile_id = clsid_to_string(&GUID_LEXI_PROFILE);
    let tip_key = format!(
        "SOFTWARE\\Microsoft\\CTF\\TIP\\{}",
        clsid_str
    );
    let lang_profile_key = format!(
        "SOFTWARE\\Microsoft\\CTF\\TIP\\{}\\LanguageProfile\\0x00000804\\{}",
        clsid_str, profile_id
    );

    if !set_reg_value(
        HKEY_LOCAL_MACHINE,
        &tip_key,
        "",
        "Lexi Input Method",
    ) {
        return HRESULT(-0x7ff8ffffi32);
    }

    if !set_reg_value(
        HKEY_LOCAL_MACHINE,
        &tip_key,
        "Display Description",
        "Lexi Input Method",
    ) {
        return HRESULT(-0x7ff8ffffi32);
    }

    if !set_reg_value(
        HKEY_LOCAL_MACHINE,
        &lang_profile_key,
        "",
        "Lexi Pinyin",
    ) {
        return HRESULT(-0x7ff8ffffi32);
    }

    HRESULT(0)
}

fn unregister_ime() -> HRESULT {
    let clsid_str = clsid_to_string(&CLSID_LEXI_IME);

    let tip_key = format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{}", clsid_str);
    let tip_key_u16: Vec<u16> = tip_key.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let _ = windows::Win32::System::Registry::RegDeleteTreeW(
            HKEY_LOCAL_MACHINE,
            PCWSTR::from_raw(tip_key_u16.as_ptr()),
        );
    }

    let com_key = format!("SOFTWARE\\Classes\\CLSID\\{}", clsid_str);
    let com_key_u16: Vec<u16> = com_key.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let _ = windows::Win32::System::Registry::RegDeleteTreeW(
            HKEY_CLASSES_ROOT,
            PCWSTR::from_raw(com_key_u16.as_ptr()),
        );
    }

    HRESULT(0)
}

#[implement(IClassFactory)]
struct LexiClassFactory {}

impl LexiClassFactory {
    fn new() -> Self {
        LexiClassFactory {}
    }
}

#[allow(non_snake_case)]
impl IClassFactory_Impl for LexiClassFactory_Impl {
    fn CreateInstance(
        &self,
        _outer: Option<&IUnknown>,
        riid: *const GUID,
        ppv: *mut *mut std::ffi::c_void,
    ) -> HRESULT {
        if ppv.is_null() {
            return HRESULT(-0x7ff8ffffi32);
        }
        unsafe { *ppv = std::ptr::null_mut() };

        let service = text_service::LexiTextService::new();
        let unknown: IUnknown = service.into();

        let hr = unknown.query(riid, ppv as *mut *mut std::ffi::c_void);
        if hr.is_ok() {
            std::mem::forget(unknown);
            LEXI_ACTIVE.store(true, Ordering::SeqCst);
        }
        hr
    }

    fn LockServer(&self, _fLock: BOOL) -> HRESULT {
        HRESULT(0)
    }
}
