mod ffi;

use self::ffi::{RimeApi, RimeContext, RimeLibrary, RimeSessionId, RimeStatus, RimeTraits, RimeContextWrapper};
use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::sync::Mutex;
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateData {
    pub text: String,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextData {
    pub preedit: String,
    pub candidates: Vec<CandidateData>,
    pub page_no: i32,
    pub is_last_page: bool,
    pub highlighted_index: i32,
    pub commit_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusData {
    pub is_composing: bool,
    pub is_ascii_mode: bool,
}

pub enum KeyResult {
    Handled(ContextData),
    NotHandled,
}

struct EngineInner {
    library: Option<RimeLibrary>,
    session_id: Option<RimeSessionId>,
    initialized: bool,
}

pub struct RimeEngine {
    inner: Mutex<EngineInner>,
}

unsafe impl Send for RimeEngine {}
unsafe impl Sync for RimeEngine {}

impl RimeEngine {
    pub fn new() -> Self {
        RimeEngine {
            inner: Mutex::new(EngineInner {
                library: None,
                session_id: None,
                initialized: false,
            }),
        }
    }

    pub fn initialize(&self, app_handle: &AppHandle) -> Result<(), String> {
        let mut inner = self.inner.lock().map_err(|_| "Mutex poisoned".to_string())?;

        if inner.initialized {
            return Ok(());
        }

        let library = RimeLibrary::load().ok_or("Failed to load librime.dll")?;
        let api = library.api().ok_or("RIME API not available")?;

        let resource_dir = app_handle
            .path()
            .resource_dir()
            .map_err(|e| format!("Failed to get resource dir: {}", e))?;

        let shared_dir = resource_dir.join("rime");
        let shared_path = CString::new(
            shared_dir
                .to_str()
                .ok_or("Invalid shared dir path")?,
        )
        .map_err(|e| format!("CString error: {}", e))?;

        let local_app_data = std::env::var("LOCALAPPDATA")
            .map_err(|_| "LOCALAPPDATA not set".to_string())?;
        let user_data_dir = std::path::Path::new(&local_app_data).join("Lexi").join("rime");
        std::fs::create_dir_all(&user_data_dir)
            .map_err(|e| format!("Failed to create user data dir: {}", e))?;

        let user_path = CString::new(
            user_data_dir
                .to_str()
                .ok_or("Invalid user data dir path")?,
        )
        .map_err(|e| format!("CString error: {}", e))?;

        let app_name = CString::new("Lexi")
            .map_err(|e| format!("CString error: {}", e))?;

        let mut traits = RimeTraits {
            data_size: std::mem::size_of::<RimeTraits>() as std::ffi::c_int,
            shared_data_dir: shared_path.as_ptr(),
            user_data_dir: user_path.as_ptr(),
            distribution_name: std::ptr::null(),
            distribution_code_name: std::ptr::null(),
            distribution_version: std::ptr::null(),
            app_name: app_name.as_ptr(),
            modules: std::ptr::null(),
            min_log_level: 0,
            log_dir: std::ptr::null(),
            prebuilt_data_dir: std::ptr::null(),
            staging_dir: std::ptr::null(),
        };

        unsafe {
            if let Some(setup) = api.setup {
                setup(&traits);
            }
            if let Some(init) = api.initialize {
                init(&traits);
            }
        }

        inner.library = Some(library);
        inner.initialized = true;

        Ok(())
    }

    fn ensure_session(&self, inner: &mut EngineInner) -> Result<RimeSessionId, String> {
        if let Some(sid) = inner.session_id {
            return Ok(sid);
        }

        let api = inner
            .library
            .as_ref()
            .ok_or("RIME not initialized")?
            .api()
            .ok_or("API not available")?;

        let sid = unsafe {
            (api.create_session.ok_or("create_session not available")?)()
        };

        if sid == 0 {
            return Err("Failed to create RIME session".into());
        }

        inner.session_id = Some(sid);
        Ok(sid)
    }

    fn read_context(&self, sid: RimeSessionId, api: &RimeApi) -> Result<Option<ContextData>, String> {
        let mut ctx = RimeContext {
            data_size: std::mem::size_of::<RimeContext>() as std::ffi::c_int,
            composition: unsafe { std::mem::zeroed() },
            menu: unsafe { std::mem::zeroed() },
            commit_text_preview: std::ptr::null(),
            select_labels: std::ptr::null(),
        };

        let has_context = unsafe {
            (api.get_context.ok_or("get_context not available")?)(sid, &mut ctx)
        };

        if !has_context {
            return Ok(None);
        }

        let wrapper = RimeContextWrapper(ctx);

        let preedit = wrapper.preedit().to_string();
        let candidates: Vec<CandidateData> = wrapper
            .candidates()
            .into_iter()
            .map(|(text, comment)| CandidateData { text, comment })
            .collect();

        let mut commit = ffi::RimeCommit {
            data_size: std::mem::size_of::<ffi::RimeCommit>() as std::ffi::c_int,
            text: std::ptr::null(),
        };

        let commit_text = if unsafe { (api.get_commit.ok_or("get_commit not available")?)(sid, &mut commit) } {
            let text = if commit.text.is_null() {
                String::new()
            } else {
                unsafe { std::ffi::CStr::from_ptr(commit.text).to_string_lossy().into_owned() }
            };
            unsafe { (api.free_commit.unwrap())(&mut commit) };
            text
        } else {
            String::new()
        };

        let result = ContextData {
            preedit,
            candidates,
            page_no: wrapper.page_no(),
            is_last_page: wrapper.is_last_page(),
            highlighted_index: wrapper.highlighted_index(),
            commit_text,
        };

        unsafe {
            (api.free_context.ok_or("free_context not available")?)(&mut ctx);
        }

        Ok(Some(result))
    }

    pub fn process_key(&self, keycode: i32, modifiers: i32) -> Result<KeyResult, String> {
        let mut inner = self.inner.lock().map_err(|_| "Mutex poisoned".to_string())?;
        let sid = self.ensure_session(&mut inner)?;

        let api = inner
            .library
            .as_ref()
            .ok_or("RIME not initialized")?
            .api()
            .ok_or("API not available")?;

        let processed = unsafe {
            (api.process_key.ok_or("process_key not available")?)(sid, keycode, modifiers)
        };

        if !processed {
            return Ok(KeyResult::NotHandled);
        }

        Ok(match self.read_context(sid, api)? {
            Some(ctx) => KeyResult::Handled(ctx),
            None => KeyResult::Handled(ContextData {
                preedit: String::new(),
                candidates: Vec::new(),
                page_no: 0,
                is_last_page: true,
                highlighted_index: 0,
                commit_text: String::new(),
            }),
        })
    }

    pub fn select_candidate(&self, index: i32) -> Result<Option<ContextData>, String> {
        let mut inner = self.inner.lock().map_err(|_| "Mutex poisoned".to_string())?;
        let sid = self.ensure_session(&mut inner)?;

        let api = inner
            .library
            .as_ref()
            .ok_or("RIME not initialized")?
            .api()
            .ok_or("API not available")?;

        let mut ctx = RimeContext {
            data_size: std::mem::size_of::<RimeContext>() as std::ffi::c_int,
            composition: unsafe { std::mem::zeroed() },
            menu: unsafe { std::mem::zeroed() },
            commit_text_preview: std::ptr::null(),
            select_labels: std::ptr::null(),
        };

        let has_context = unsafe {
            (api.get_context.ok_or("get_context not available")?)(sid, &mut ctx)
        };

        if !has_context {
            return Err("No context available".into());
        }

        let num_candidates = ctx.menu.num_candidates;
        if index < 0 || index >= num_candidates {
            unsafe { (api.free_context.ok_or("free_context not available")?)(&mut ctx) };
            return Err(format!("Invalid candidate index: {} (max: {})", index, num_candidates - 1));
        }

        unsafe { (api.free_context.ok_or("free_context not available")?)(&mut ctx) };

        let selected = unsafe {
            (api.select_candidate.ok_or("select_candidate not available")?)(sid, index)
        };

        if !selected {
            return Err("RIME select_candidate failed".into());
        }

        self.read_context(sid, api)
    }

    pub fn clear_composition(&self) -> Result<(), String> {
        let mut inner = self.inner.lock().map_err(|_| "Mutex poisoned".to_string())?;
        let sid = self.ensure_session(&mut inner)?;

        let api = inner
            .library
            .as_ref()
            .ok_or("RIME not initialized")?
            .api()
            .ok_or("API not available")?;

        unsafe {
            (api.clear_composition.ok_or("clear_composition not available")?)(sid);
        }

        Ok(())
    }

    pub fn get_status(&self) -> Result<StatusData, String> {
        let mut inner = self.inner.lock().map_err(|_| "Mutex poisoned".to_string())?;
        let sid = self.ensure_session(&mut inner)?;

        let api = inner
            .library
            .as_ref()
            .ok_or("RIME not initialized")?
            .api()
            .ok_or("API not available")?;

        let mut status = RimeStatus {
            data_size: std::mem::size_of::<RimeStatus>() as std::ffi::c_int,
            schema_id: std::ptr::null(),
            schema_name: std::ptr::null(),
            is_disabled: false,
            is_composing: false,
            is_ascii_mode: false,
            is_full_shape: false,
            is_simplified: false,
            is_traditional: false,
            is_ascii_punct: false,
        };

        let ok = unsafe {
            (api.get_status.ok_or("get_status not available")?)(sid, &mut status)
        };

        if !ok {
            return Err("Failed to get RIME status".into());
        }

        let result = StatusData {
            is_composing: status.is_composing,
            is_ascii_mode: status.is_ascii_mode,
        };

        unsafe {
            (api.free_status.ok_or("free_status not available")?)(&mut status);
        }

        Ok(result)
    }

    fn destroy_inner(&self) {
        let mut inner = match self.inner.lock() {
            Ok(i) => i,
            Err(_) => return,
        };

        if let Some(sid) = inner.session_id.take() {
            if let Some(ref library) = inner.library {
                if let Some(api) = library.api() {
                    unsafe {
                        (api.destroy_session.unwrap())(sid);
                    }
                }
            }
        }

        if inner.initialized {
            if let Some(ref library) = inner.library {
                if let Some(api) = library.api() {
                    unsafe {
                        (api.finalize.unwrap())();
                    }
                }
            }
            inner.initialized = false;
        }

        inner.library.take();
    }
}

impl Drop for RimeEngine {
    fn drop(&mut self) {
        self.destroy_inner();
    }
}
