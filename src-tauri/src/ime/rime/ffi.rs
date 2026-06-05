use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};

pub type RimeSessionId = c_int;
pub type Bool = c_int;

pub const TRUE: Bool = 1;
pub const FALSE: Bool = 0;

#[repr(C)]
pub struct RimeTraits {
    pub data_size: usize,
    pub shared_data_dir: *const c_char,
    pub user_data_dir: *const c_char,
    pub distribution_name: *const c_char,
    pub distribution_code_name: *const c_char,
    pub distribution_version: *const c_char,
    pub app_name: *const c_char,
    pub preset_data_dir: *const c_char,
    pub staging_dir: *const c_char,
    pub log_dir: *const c_char,
    pub min_log_level: c_int,
    pub extra: *mut c_void,
}

#[repr(C)]
pub struct RimeCommit {
    pub data_size: usize,
    pub text: *const c_char,
}

#[repr(C)]
pub struct RimeCandidate {
    pub text: *const c_char,
    pub comment: *const c_char,
    pub reserved: [*mut c_void; 16usize],
}

#[repr(C)]
pub struct RimeMenu {
    pub page_size: c_int,
    pub page_no: c_int,
    pub is_last_page: Bool,
    pub highlighted_candidate_index: c_int,
    pub num_candidates: c_int,
    pub candidates: *mut RimeCandidate,
    pub select_keys: *const c_char,
}

#[repr(C)]
pub struct RimeContext {
    pub data_size: usize,
    pub composition: RimeComposition,
    pub menu: RimeMenu,
    pub commit_text_preview: *const c_char,
    pub select_labels: *mut *mut c_char,
    pub reserved: [*mut c_void; 16usize],
}

#[repr(C)]
pub struct RimeComposition {
    pub length: usize,
    pub cursor_pos: usize,
    pub sel_start: usize,
    pub sel_end: usize,
    pub preedit: *const c_char,
}

#[repr(C)]
pub struct RimeStatus {
    pub data_size: usize,
    pub schema_id: *const c_char,
    pub schema_name: *const c_char,
    pub is_disabled: Bool,
    pub is_composing: Bool,
    pub is_ascii_mode: Bool,
    pub is_full_shape: Bool,
    pub is_simplified: Bool,
    pub is_traditional: Bool,
    pub is_ascii_punct: Bool,
    pub reserved: [*mut c_void; 32usize],
}

#[repr(C)]
pub struct RimeApi {
    pub api_version: c_int,
    pub setup: Option<unsafe extern "C" fn(traits: *mut RimeTraits)>,
    pub initialize: Option<unsafe extern "C" fn(traits: *mut RimeTraits)>,
    pub finalize: Option<unsafe extern "C" fn()>,
    pub start_session: Option<unsafe extern "C" fn() -> RimeSessionId>,
    pub find_session: Option<unsafe extern "C" fn(session_id: RimeSessionId) -> Bool>,
    pub destroy_session: Option<unsafe extern "C" fn(session_id: RimeSessionId)>,
    pub process_key: Option<unsafe extern "C" fn(session_id: RimeSessionId, keycode: c_int, mask: c_int) -> Bool>,
    pub commit_composition: Option<unsafe extern "C" fn(session_id: RimeSessionId)>,
    pub clear_composition: Option<unsafe extern "C" fn(session_id: RimeSessionId)>,
    pub get_commit: Option<unsafe extern "C" fn(session_id: RimeSessionId, commit: *mut RimeCommit) -> Bool>,
    pub free_commit: Option<unsafe extern "C" fn(commit: *mut RimeCommit)>,
    pub get_context: Option<unsafe extern "C" fn(session_id: RimeSessionId, ctx: *mut RimeContext) -> Bool>,
    pub free_context: Option<unsafe extern "C" fn(ctx: *mut RimeContext)>,
    pub select_candidate: Option<unsafe extern "C" fn(session_id: RimeSessionId, index: c_int) -> Bool>,
    pub get_status: Option<unsafe extern "C" fn(session_id: RimeSessionId, status: *mut RimeStatus) -> Bool>,
    pub free_status: Option<unsafe extern "C" fn(status: *mut RimeStatus)>,
    pub create_session: Option<unsafe extern "C" fn() -> RimeSessionId>,
    pub is_maintenance_mode: Option<unsafe extern "C" fn() -> Bool>,
}

impl RimeCommit {
    pub fn text(&self) -> &str {
        if self.text.is_null() {
            return "";
        }
        unsafe { CStr::from_ptr(self.text).to_str().unwrap_or("") }
    }
}

impl RimeContext {
    pub fn preedit(&self) -> &str {
        if self.composition.preedit.is_null() {
            return "";
        }
        unsafe { CStr::from_ptr(self.composition.preedit).to_str().unwrap_or("") }
    }

    pub fn candidates(&self) -> Vec<(String, String)> {
        let mut result = Vec::new();
        if self.menu.candidates.is_null() {
            return result;
        }
        let num = self.menu.num_candidates as usize;
        for i in 0..num {
            let cand = unsafe { &*self.menu.candidates.add(i) };
            if !cand.text.is_null() {
                let text = unsafe { CStr::from_ptr(cand.text).to_str().unwrap_or("") };
                let comment = if cand.comment.is_null() {
                    ""
                } else {
                    unsafe { CStr::from_ptr(cand.comment).to_str().unwrap_or("") }
                };
                result.push((text.to_string(), comment.to_string()));
            }
        }
        result
    }

    pub fn page_no(&self) -> i32 {
        self.menu.page_no
    }

    pub fn is_last_page(&self) -> bool {
        self.menu.is_last_page == TRUE
    }

    pub fn highlighted_index(&self) -> i32 {
        self.menu.highlighted_candidate_index
    }
}

pub struct RimeLibrary {
    lib: *mut std::ffi::c_void,
    api: Option<&'static RimeApi>,
}

impl RimeLibrary {
    pub fn load() -> Option<Self> {
        #[cfg(windows)]
        {
            type HMODULE = *mut std::ffi::c_void;
            type FARPROC = Option<unsafe extern "system" fn() -> isize>;

            extern "system" {
                fn LoadLibraryA(name: *const u8) -> HMODULE;
                fn GetProcAddress(module: HMODULE, name: *const u8) -> FARPROC;
            }

            let lib_name = CString::new("librime.dll").ok()?;
            let get_api_name = CString::new("rime_get_api").ok()?;

            let lib = unsafe { LoadLibraryA(lib_name.as_ptr() as *const u8) };
            if lib.is_null() {
                return None;
            }

            let func_ptr = unsafe { GetProcAddress(lib, get_api_name.as_ptr() as *const u8) }?;
            let get_api_fn: unsafe extern "C" fn() -> *const RimeApi =
                unsafe { std::mem::transmute(func_ptr) };

            let api_ptr = unsafe { (get_api_fn)() };
            if api_ptr.is_null() {
                return None;
            }

            let api = unsafe { Some(&*api_ptr) };

            Some(RimeLibrary {
                lib,
                api,
            })
        }

        #[cfg(not(windows))]
        {
            None
        }
    }

    pub fn api(&self) -> Option<&'static RimeApi> {
        self.api
    }
}

impl Drop for RimeLibrary {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            if !self.lib.is_null() {
                extern "system" {
                    fn FreeLibrary(module: *mut std::ffi::c_void) -> i32;
                }
                unsafe {
                    FreeLibrary(self.lib);
                }
            }
        }
    }
}
