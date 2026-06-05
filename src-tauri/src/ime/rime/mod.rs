mod ffi;

use std::ffi::CStr;
use std::ffi::CString;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::AppHandle;
use tauri::Manager;

use self::ffi::*;

pub struct RimeEngine {
    library: Mutex<Option<RimeLibrary>>,
    session_id: Mutex<Option<RimeSessionId>>,
    initialized: Mutex<bool>,
}

unsafe impl Send for RimeEngine {}
unsafe impl Sync for RimeEngine {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CandidateData {
    pub text: String,
    pub comment: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContextData {
    pub preedit: String,
    pub candidates: Vec<CandidateData>,
    pub page_no: i32,
    pub is_last_page: bool,
    pub highlighted_index: i32,
    pub commit_text: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatusData {
    pub is_composing: bool,
    pub is_ascii_mode: bool,
}

impl RimeEngine {
    pub fn new() -> Self {
        RimeEngine {
            library: Mutex::new(None),
            session_id: Mutex::new(None),
            initialized: Mutex::new(false),
        }
    }

    pub fn initialize(&self, app_handle: &AppHandle) -> Result<(), String> {
        let mut init = self.initialized.lock().map_err(|e| e.to_string())?;
        if *init {
            return Ok(());
        }

        let rime_lib = RimeLibrary::load().ok_or_else(|| {
            "librime.dll not found. Place librime.dll in the app directory.".to_string()
        })?;

        let api = rime_lib.api().ok_or("Failed to get RIME API")?;

        let data_dir = app_handle
            .path()
            .resource_dir()
            .map_err(|e| e.to_string())?
            .join("rime");

        let user_data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Lexi")
            .join("rime");

        std::fs::create_dir_all(&user_data_dir).map_err(|e| e.to_string())?;

        let shared_data = CString::new(
            data_dir.to_str().ok_or("invalid path")?,
        )
        .map_err(|e| e.to_string())?;
        let user_data = CString::new(
            user_data_dir.to_str().ok_or("invalid path")?,
        )
        .map_err(|e| e.to_string())?;
        let dist_name = CString::new("Lexi").map_err(|e| e.to_string())?;
        let dist_code = CString::new("lexi").map_err(|e| e.to_string())?;
        let dist_ver = CString::new("0.1.0").map_err(|e| e.to_string())?;
        let app_name = CString::new("lexi.inputmethod").map_err(|e| e.to_string())?;

        let mut traits = RimeTraits {
            data_size: std::mem::size_of::<RimeTraits>(),
            shared_data_dir: shared_data.as_ptr(),
            user_data_dir: user_data.as_ptr(),
            distribution_name: dist_name.as_ptr(),
            distribution_code_name: dist_code.as_ptr(),
            distribution_version: dist_ver.as_ptr(),
            app_name: app_name.as_ptr(),
            preset_data_dir: std::ptr::null(),
            staging_dir: std::ptr::null(),
            log_dir: std::ptr::null(),
            min_log_level: 0,
            extra: std::ptr::null_mut(),
        };

        unsafe {
            if let Some(setup) = api.setup {
                setup(&mut traits);
            }
            if let Some(initialize) = api.initialize {
                initialize(&mut traits);
            }
        }

        self.library.lock().map_err(|e| e.to_string())?.replace(rime_lib);
        *init = true;
        Ok(())
    }

    fn with_api<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&RimeApi) -> Result<R, String>,
    {
        let lib = self.library.lock().map_err(|e| e.to_string())?;
        let api = lib
            .as_ref()
            .and_then(|l| l.api())
            .ok_or("RIME engine not initialized".to_string())?;
        f(api)
    }

    pub fn ensure_session(&self) -> Result<RimeSessionId, String> {
        let mut session = self.session_id.lock().map_err(|e| e.to_string())?;
        match *session {
            Some(sid) => Ok(sid),
            None => {
                let sid = self.with_api(|api| unsafe {
                    let start_fn = api
                        .start_session
                        .or(api.create_session)
                        .ok_or("No session creation function".to_string())?;
                    let sid = start_fn();
                    if sid == 0 {
                        Err("Failed to create RIME session".to_string())
                    } else {
                        Ok(sid)
                    }
                })?;
                *session = Some(sid);
                Ok(sid)
            }
        }
    }

    pub fn process_key(&self, keycode: i32, modifiers: i32) -> Result<Option<ContextData>, String> {
        let sid = self.ensure_session()?;
        self.with_api(|api| unsafe {
            let process_fn = api
                .process_key
                .ok_or("No process_key function")?;
            let processed = process_fn(sid, keycode, modifiers);
            if processed == FALSE {
                return Ok(None);
            }
            self.read_context(sid, api)
        })
    }

    pub fn select_candidate(&self, index: i32) -> Result<Option<ContextData>, String> {
        let sid = self.ensure_session()?;
        self.with_api(|api| unsafe {
            if let Some(select) = api.select_candidate {
                select(sid, index);
            }

            let mut commit = RimeCommit {
                data_size: std::mem::size_of::<RimeCommit>(),
                text: std::ptr::null(),
            };

            let has_commit = api
                .get_commit
                .map(|f| f(sid, &mut commit))
                .unwrap_or(FALSE);

            if has_commit != FALSE && !commit.text.is_null() {
                let text = CStr::from_ptr(commit.text)
                    .to_str()
                    .unwrap_or("")
                    .to_string();
                if let Some(free) = api.free_commit {
                    free(&mut commit);
                }

                let context = self.read_context(sid, api)?;
                let mut cd = context.unwrap_or(ContextData {
                    preedit: String::new(),
                    candidates: Vec::new(),
                    page_no: 0,
                    is_last_page: true,
                    highlighted_index: 0,
                    commit_text: String::new(),
                });
                cd.commit_text = text;
                return Ok(Some(cd));
            }

            self.read_context(sid, api)
        })
    }

    pub fn clear_composition(&self) -> Result<(), String> {
        let sid = self.ensure_session()?;
        self.with_api(|api| unsafe {
            if let Some(clear) = api.clear_composition {
                clear(sid);
            }
            Ok(())
        })
    }

    unsafe fn read_context(
        &self,
        sid: RimeSessionId,
        api: &RimeApi,
    ) -> Result<Option<ContextData>, String> {
        let mut ctx = RimeContext {
            data_size: std::mem::size_of::<RimeContext>(),
            composition: std::mem::zeroed(),
            menu: std::mem::zeroed(),
            commit_text_preview: std::ptr::null(),
            select_labels: std::ptr::null_mut(),
            reserved: [std::ptr::null_mut(); 16],
        };

        let got = api
            .get_context
            .map(|f| f(sid, &mut ctx))
            .unwrap_or(FALSE);

        if got == FALSE || ctx.candidates().is_empty() {
            if let Some(free) = api.free_context {
                free(&mut ctx);
            }
            return Ok(None);
        }

        let preedit = ctx.preedit().to_string();
        let candidates: Vec<CandidateData> = ctx
            .candidates()
            .into_iter()
            .map(|(text, comment)| CandidateData { text, comment })
            .collect();
        let page_no = ctx.page_no();
        let is_last_page = ctx.is_last_page();
        let highlighted = ctx.highlighted_index();

        if let Some(free) = api.free_context {
            free(&mut ctx);
        }

        Ok(Some(ContextData {
            preedit,
            candidates,
            page_no,
            is_last_page,
            highlighted_index: highlighted,
            commit_text: String::new(),
        }))
    }

    pub fn get_status(&self) -> Result<StatusData, String> {
        let sid = self.ensure_session()?;
        self.with_api(|api| unsafe {
            let mut status = RimeStatus {
                data_size: std::mem::size_of::<RimeStatus>(),
                schema_id: std::ptr::null(),
                schema_name: std::ptr::null(),
                is_disabled: FALSE,
                is_composing: FALSE,
                is_ascii_mode: FALSE,
                is_full_shape: FALSE,
                is_simplified: FALSE,
                is_traditional: FALSE,
                is_ascii_punct: FALSE,
                reserved: [std::ptr::null_mut(); 32],
            };

            let got = api
                .get_status
                .map(|f| f(sid, &mut status))
                .unwrap_or(FALSE);

            let result = if got != FALSE {
                StatusData {
                    is_composing: status.is_composing != FALSE,
                    is_ascii_mode: status.is_ascii_mode != FALSE,
                }
            } else {
                StatusData {
                    is_composing: false,
                    is_ascii_mode: false,
                }
            };

            if let Some(free) = api.free_status {
                free(&mut status);
            }

            Ok(result)
        })
    }

    pub fn destroy_session(&self) -> Result<(), String> {
        let mut session = self.session_id.lock().map_err(|e| e.to_string())?;
        if let Some(sid) = session.take() {
            self.with_api(|api| unsafe {
                if let Some(destroy) = api.destroy_session {
                    destroy(sid);
                }
                Ok(())
            })?;
        }
        Ok(())
    }

    pub fn destroy(&self) -> Result<(), String> {
        self.destroy_session()?;
        self.with_api(|api| unsafe {
            if let Some(finalize) = api.finalize {
                finalize();
            }
            Ok(())
        })?;
        let mut init = self.initialized.lock().map_err(|e| e.to_string())?;
        *init = false;
        Ok(())
    }
}
