use std::ffi::{c_void, CStr, CString};
use std::sync::Mutex;

pub type RimeSessionId = u32;

#[repr(C)]
pub struct RimeTraits {
    pub data_size: std::ffi::c_int,
    pub shared_data_dir: *const i8,
    pub user_data_dir: *const i8,
    pub distribution_name: *const i8,
    pub distribution_code_name: *const i8,
    pub distribution_version: *const i8,
    pub app_name: *const i8,
    pub modules: *const *const i8,
    pub min_log_level: std::ffi::c_int,
    pub log_dir: *const i8,
    pub prebuilt_data_dir: *const i8,
    pub staging_dir: *const i8,
}

#[repr(C)]
pub struct RimeCommit {
    pub data_size: std::ffi::c_int,
    pub text: *const i8,
}

#[repr(C)]
pub struct RimeCandidate {
    pub data_size: std::ffi::c_int,
    pub text: *const i8,
    pub comment: *const i8,
    pub reserved: [*mut c_void; 4usize],
}

#[repr(C)]
pub struct RimeMenu {
    pub data_size: std::ffi::c_int,
    pub num_candidates: std::ffi::c_int,
    pub candidates: *const RimeCandidate,
    pub page_no: std::ffi::c_int,
    pub page_size: std::ffi::c_int,
    pub current_page_start_index: std::ffi::c_int,
    pub highlighted_candidate_index: std::ffi::c_int,
    pub is_last_page: bool,
    pub select_keys: *const i8,
    pub reserved: [*mut c_void; 4usize],
}

#[repr(C)]
pub struct RimeComposition {
    pub data_size: std::ffi::c_int,
    pub preedit: *const i8,
    pub sel_start: std::ffi::c_int,
    pub sel_end: std::ffi::c_int,
    pub cursor_pos: std::ffi::c_int,
    pub use_preedit: bool,
}

#[repr(C)]
pub struct RimeContext {
    pub data_size: std::ffi::c_int,
    pub composition: RimeComposition,
    pub menu: RimeMenu,
    pub commit_text_preview: *const i8,
    pub select_labels: *const i8,
}

#[repr(C)]
pub struct RimeStatus {
    pub data_size: std::ffi::c_int,
    pub schema_id: *const i8,
    pub schema_name: *const i8,
    pub is_disabled: bool,
    pub is_composing: bool,
    pub is_ascii_mode: bool,
    pub is_full_shape: bool,
    pub is_simplified: bool,
    pub is_traditional: bool,
    pub is_ascii_punct: bool,
}

pub type RimeApiGetApiFn = unsafe extern "C" fn() -> *const RimeApi;

#[repr(C)]
pub struct RimeApi {
    pub data_size: std::ffi::c_int,
    pub setup: Option<unsafe extern "C" fn(*const RimeTraits) -> ()>,
    pub initialize: Option<unsafe extern "C" fn(*const RimeTraits) -> ()>,
    pub finalize: Option<unsafe extern "C" fn() -> ()>,
    pub is_maintenance_mode: Option<unsafe extern "C" fn() -> bool>,
    pub create_session: Option<unsafe extern "C" fn() -> RimeSessionId>,
    pub find_session: Option<unsafe extern "C" fn(*const i8) -> RimeSessionId>,
    pub destroy_session: Option<unsafe extern "C" fn(RimeSessionId) -> ()>,
    pub process_key: Option<unsafe extern "C" fn(RimeSessionId, std::ffi::c_int, std::ffi::c_int) -> bool>,
    pub commit_composition: Option<unsafe extern "C" fn(RimeSessionId) -> ()>,
    pub clear_composition: Option<unsafe extern "C" fn(RimeSessionId) -> ()>,
    pub get_commit: Option<unsafe extern "C" fn(RimeSessionId, *mut RimeCommit) -> bool>,
    pub free_commit: Option<unsafe extern "C" fn(*mut RimeCommit) -> ()>,
    pub get_context: Option<unsafe extern "C" fn(RimeSessionId, *mut RimeContext) -> bool>,
    pub free_context: Option<unsafe extern "C" fn(*mut RimeContext) -> ()>,
    pub get_status: Option<unsafe extern "C" fn(RimeSessionId, *mut RimeStatus) -> bool>,
    pub free_status: Option<unsafe extern "C" fn(*mut RimeStatus) -> ()>,
    pub set_option: Option<unsafe extern "C" fn(RimeSessionId, *const i8, bool) -> ()>,
    pub set_property: Option<unsafe extern "C" fn(RimeSessionId, *const i8, *const i8) -> ()>,
    pub get_schema_list: Option<unsafe extern "C" fn(...) -> ()>,
    pub free_schema_list: Option<unsafe extern "C" fn(...) -> ()>,
    pub select_schema: Option<unsafe extern "C" fn(RimeSessionId, *const i8) -> bool>,
    pub select_candidate: Option<unsafe extern "C" fn(RimeSessionId, std::ffi::c_int) -> bool>,
    pub candidate_at: Option<unsafe extern "C" fn(RimeSessionId, std::ffi::c_int, *mut RimeCandidate) -> bool>,
    pub free_candidate: Option<unsafe extern "C" fn(*mut RimeCandidate) -> ()>,
    pub get_candidate_list: Option<unsafe extern "C" fn(RimeSessionId, *mut RimeCandidate) -> ()>,
    pub free_candidate_list: Option<unsafe extern "C" fn(*mut RimeCandidate) -> ()>,
    pub set_notification_handler: Option<unsafe extern "C" fn(...) -> ()>,
    pub get_version: Option<unsafe extern "C" fn() -> *const i8>,
    pub open_api: Option<unsafe extern "C" fn() -> ()>,
    pub get_notification: Option<unsafe extern "C" fn(...) -> ()>,
    pub free_notification: Option<unsafe extern "C" fn(...) -> ()>,
    pub start_maintenance: Option<unsafe extern "C" fn(...) -> ()>,
    pub is_ready: Option<unsafe extern "C" fn() -> bool>,
    pub deployer_initialize: Option<unsafe extern "C" fn(...) -> ()>,
    pub deploy: Option<unsafe extern "C" fn() -> bool>,
    pub prebuild: Option<unsafe extern "C" fn(...) -> ()>,
    pub deploy_config_file: Option<unsafe extern "C" fn(...) -> ()>,
    pub sync_user_data: Option<unsafe extern "C" fn() -> ()>,
    pub get_shared_data_dir: Option<unsafe extern "C" fn() -> *mut i8>,
    pub get_user_data_dir: Option<unsafe extern "C" fn() -> *mut i8>,
    pub get_sync_dir: Option<unsafe extern "C" fn() -> *mut i8>,
    pub get_user_id: Option<unsafe extern "C" fn() -> *const i8>,
    pub get_user_id_sync_dir: Option<unsafe extern "C" fn() -> *mut i8>,
    pub set_user_id: Option<unsafe extern "C" fn(*const i8) -> ()>,
    pub get_api_version: Option<unsafe extern "C" fn() -> *const i8>,
}

pub struct RimeLibrary {
    lib: *mut c_void,
    api: Option<&'static RimeApi>,
}

unsafe impl Send for RimeLibrary {}
unsafe impl Sync for RimeLibrary {}

impl RimeLibrary {
    pub fn load() -> Option<Self> {
        let lib_name = CString::new("librime.dll").ok()?;

        let lib = unsafe { windows::Win32::System::LibraryLoader::LoadLibraryA(lib_name.as_ptr()) };
        if lib.is_invalid() {
            return None;
        }

        let func_name = CString::new("rime_get_api").ok()?;
        let func_ptr = unsafe { windows::Win32::System::LibraryLoader::GetProcAddress(lib, func_name.as_ptr()) };

        if func_ptr.is_none() {
            unsafe { let _ = windows::Win32::System::LibraryLoader::FreeLibrary(lib); }
            return None;
        }

        let get_api_fn: RimeApiGetApiFn = unsafe { std::mem::transmute(func_ptr.unwrap()) };
        let api_ptr = unsafe { (get_api_fn)() };

        if api_ptr.is_null() {
            unsafe { let _ = windows::Win32::System::LibraryLoader::FreeLibrary(lib); }
            return None;
        }

        let api = unsafe { Some(&*api_ptr) };
        Some(RimeLibrary { lib: lib.as_ptr() as *mut c_void, api })
    }

    pub fn api(&self) -> Option<&'static RimeApi> {
        self.api
    }

    pub fn with_api<F>(&self, f: F) -> Result<(), String>
    where
        F: Fn(&'static RimeApi) -> Result<(), String>,
    {
        match self.api {
            Some(api) => f(api),
            None => Err("RIME API not available".into()),
        }
    }
}

impl Drop for RimeLibrary {
    fn drop(&mut self) {
        if !self.lib.is_null() {
            unsafe {
                let _ = windows::Win32::System::LibraryLoader::FreeLibrary(
                    windows::Win32::Foundation::HMODULE(self.lib as _),
                );
            }
        }
    }
}

pub struct RimeContextWrapper(pub RimeContext);

impl RimeContextWrapper {
    pub fn preedit(&self) -> &str {
        if self.0.composition.preedit.is_null() {
            return "";
        }
        unsafe { CStr::from_ptr(self.0.composition.preedit).to_str().unwrap_or("") }
    }

    pub fn candidates(&self) -> Vec<(String, String)> {
        let num = self.0.menu.num_candidates;
        if num <= 0 {
            return Vec::new();
        }
        let count = num as usize;
        let mut result = Vec::with_capacity(count);
        for i in 0..count {
            unsafe {
                let cand = &*self.0.menu.candidates.add(i);
                let text = CStr::from_ptr(cand.text).to_string_lossy().into_owned();
                let comment = if cand.comment.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(cand.comment).to_string_lossy().into_owned()
                };
                result.push((text, comment));
            }
        }
        result
    }

    pub fn page_no(&self) -> i32 {
        self.0.menu.page_no
    }

    pub fn is_last_page(&self) -> bool {
        self.0.menu.is_last_page
    }

    pub fn highlighted_index(&self) -> i32 {
        self.0.menu.highlighted_candidate_index
    }
}
