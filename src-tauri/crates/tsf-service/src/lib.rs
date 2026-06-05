mod pipe_client;
mod text_service;

use std::sync::atomic::{AtomicBool, Ordering};
use windows::core::{implement, GUID, HRESULT};
use windows::Win32::Foundation::{BOOL, HINSTANCE};
use windows::Win32::System::Com::{IClassFactory, IUnknown};
use windows::Win32::System::LibraryLoader::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
use windows::Win32::UI::TextServices::ITfTextInputProcessor;

pub const CLSID_LEXI_IME: GUID = GUID::from_u128(0x12340001_0000_0000_C000_000000000046);

static DLL_INSTANCE: std::sync::OnceLock<HINSTANCE> = std::sync::OnceLock::new();
static LEXI_ACTIVE: AtomicBool = AtomicBool::new(false);

#[no_mangle]
pub extern "system" fn DllMain(
    hinst: HINSTANCE,
    reason: u32,
    _reserved: *mut std::ffi::c_void,
) -> BOOL {
    match reason {
        DLL_PROCESS_ATTACH => {
            DLL_INSTANCE.set(hinst).ok();
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
    HRESULT(0)
}

fn unregister_ime() -> HRESULT {
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
