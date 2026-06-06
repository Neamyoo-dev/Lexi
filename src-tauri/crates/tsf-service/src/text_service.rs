use crate::pipe_client;
use windows::core::{implement, HRESULT};
use windows::Win32::Foundation::{LPARAM, POINT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::TextServices::{
    ITfClientId, ITfContext, ITfEditSession, ITfEditSession_Impl, ITfInsertAtSelection,
    ITfKeyEventSink, ITfKeyEventSink_Impl, ITfSource,
    ITfTextInputProcessor, ITfTextInputProcessorEx,
    ITfTextInputProcessorEx_Impl, ITfTextInputProcessor_Impl, ITfThreadMgr,
};

use std::sync::Mutex;

const TF_ES_SYNC: u32 = 0x0002;
const TF_ES_READWRITE: u32 = 0x0004;

#[derive(serde::Serialize, serde::Deserialize)]
struct KeyEvent {
    r#type: String,
    keycode: u32,
    modifiers: u32,
}

#[implement(ITfTextInputProcessor, ITfTextInputProcessorEx)]
pub struct LexiTextService {
    client_id: Mutex<u32>,
    thread_mgr: Mutex<Option<ITfThreadMgr>>,
    active: Mutex<bool>,
    key_sink_installed: Mutex<bool>,
}

impl LexiTextService {
    pub fn new() -> Self {
        LexiTextService {
            client_id: Mutex::new(0),
            thread_mgr: Mutex::new(None),
            active: Mutex::new(false),
            key_sink_installed: Mutex::new(false),
        }
    }
}

#[allow(non_snake_case)]
impl ITfTextInputProcessor_Impl for LexiTextService_Impl {
    fn Activate(&self, ptim: &ITfThreadMgr, tid: &ITfClientId) -> HRESULT {
        let client_id = unsafe { tid.GetClientId() };

        *self.client_id.lock().unwrap() = client_id;
        *self.thread_mgr.lock().unwrap() = Some(ptim.clone());
        *self.active.lock().unwrap() = true;

        let _ = pipe_client::connect();

        self.install_key_event_sink(ptim, client_id)
    }

    fn Deactivate(&self, ptim: &ITfThreadMgr, _tid: &ITfClientId) -> HRESULT {
        self.uninstall_key_event_sink(ptim);
        *self.active.lock().unwrap() = false;
        pipe_client::disconnect();
        HRESULT(0)
    }
}

#[allow(non_snake_case)]
impl ITfTextInputProcessorEx_Impl for LexiTextService_Impl {
    fn ActivateEx(&self, ptim: &ITfThreadMgr, tid: &ITfClientId, _dwFlags: u32) -> HRESULT {
        self.Activate(ptim, tid)
    }
}

impl LexiTextService_Impl {
    fn install_key_event_sink(&self, ptim: &ITfThreadMgr, client_id: u32) -> HRESULT {
        let mut installed = self.key_sink_installed.lock().unwrap();
        if *installed {
            return HRESULT(0);
        }

        let source: Result<ITfSource, _> = ptim.cast();
        if let Ok(source) = source {
            let key_sink = LexiKeyEventSink::new(client_id);
            let unknown: windows::core::IUnknown = key_sink.into();

            let cookie = unsafe { source.AdviseSink(&ITfKeyEventSink::IID, &unknown) };

            if cookie.is_ok() {
                *installed = true;
            }
        }

        HRESULT(0)
    }

    fn uninstall_key_event_sink(&self, ptim: &ITfThreadMgr) {
        let mut installed = self.key_sink_installed.lock().unwrap();
        if !*installed {
            return;
        }

        let source: Result<ITfSource, _> = ptim.cast();
        if let Ok(source) = source {
            unsafe {
                let _ = source.UnadviseSink(&ITfKeyEventSink::IID);
            }
        }
        *installed = false;
    }
}

fn extract_modifier_mask() -> u32 {
    const RIME_SHIFT: u32 = 1;
    const RIME_CTRL: u32 = 2;
    const RIME_ALT: u32 = 4;
    const RIME_WIN: u32 = 8;

    let mut mask = 0u32;
    unsafe {
        if GetAsyncKeyState(VK_SHIFT.0 as i32) & 0x8000 != 0 {
            mask |= RIME_SHIFT;
        }
        if GetAsyncKeyState(VK_CONTROL.0 as i32) & 0x8000 != 0 {
            mask |= RIME_CTRL;
        }
        if GetAsyncKeyState(VK_MENU.0 as i32) & 0x8000 != 0 {
            mask |= RIME_ALT;
        }
        if (GetAsyncKeyState(VK_LWIN.0 as i32) | GetAsyncKeyState(VK_RWIN.0 as i32)) & 0x8000 != 0 {
            mask |= RIME_WIN;
        }
    }
    mask
}

#[implement(ITfKeyEventSink)]
struct LexiKeyEventSink {
    client_id: u32,
}

impl LexiKeyEventSink {
    fn new(client_id: u32) -> Self {
        LexiKeyEventSink { client_id }
    }

    fn send_key_event(&self, keycode: u32) -> (bool, String) {
        let modifiers = extract_modifier_mask();

        let cursor_pos = unsafe {
            let mut pt = POINT { x: 0, y: 0 };
            if windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt).as_bool() {
                Some((pt.x, pt.y))
            } else {
                None
            }
        };

        let msg = match cursor_pos {
            Some((cx, cy)) => serde_json::json!({
                "type": "keydown",
                "keycode": keycode,
                "modifiers": modifiers,
                "cursor_x": cx,
                "cursor_y": cy,
            }).to_string(),
            None => serde_json::to_string(&KeyEvent {
                r#type: "keydown".into(),
                keycode,
                modifiers,
            }).unwrap_or_default(),
        };

        match pipe_client::send_message(&msg) {
            Ok(Some(response)) => {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
                    let handled = parsed
                        .get("handled")
                        .and_then(|h| h.as_bool())
                        .unwrap_or(false);
                    return (handled, response);
                }
                (false, response)
            }
            _ => (false, String::new()),
        }
    }
}

#[implement(ITfEditSession)]
struct LexiEditSession {
    commit_text: String,
}

impl LexiEditSession {
    fn new(text: String) -> Self {
        LexiEditSession { commit_text: text }
    }
}

#[allow(non_snake_case)]
impl ITfEditSession_Impl for LexiEditSession_Impl {
    fn DoEditSession(&self, pic: &ITfContext) -> HRESULT {
        if self.commit_text.is_empty() {
            return HRESULT(0);
        }

        let insert_at_sel: Result<ITfInsertAtSelection, _> = pic.cast();
        if let Ok(insert) = insert_at_sel {
            let text_utf16: Vec<u16> = self.commit_text.encode_utf16().collect();
            unsafe {
                let hr = insert.InsertTextAtSelection(
                    None,
                    0,
                    &text_utf16[0] as *const u16 as *const u16,
                    text_utf16.len() as i32,
                    std::ptr::null_mut(),
                );
                return hr;
            }
        }

        HRESULT(0)
    }
}

const S_OK: HRESULT = HRESULT(0);
const S_FALSE: HRESULT = HRESULT(1);

#[allow(non_snake_case)]
impl ITfKeyEventSink_Impl for LexiKeyEventSink_Impl {
    fn OnKeyDown(&self, pic: &ITfContext, wParam: WPARAM, _lParam: LPARAM) -> HRESULT {
        let keycode = wParam.0 as u32;

        let (handled, response) = self.send_key_event(keycode);

        if !response.is_empty() {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
                if let Some(commit) = parsed.get("commit").and_then(|c| c.as_str()) {
                    if !commit.is_empty() {
                        let edit_session = LexiEditSession::new(commit.to_string());
                        let es: ITfEditSession = edit_session.into();
                        unsafe {
                            let _ = pic.RequestEditSession(
                                self.client_id,
                                &es,
                                TF_ES_SYNC | TF_ES_READWRITE,
                            );
                        }
                    }
                }
            }
        }

        if handled {
            S_OK
        } else {
            S_FALSE
        }
    }

    fn OnKeyUp(&self, _pic: &ITfContext, _wParam: WPARAM, _lParam: LPARAM) -> HRESULT {
        S_FALSE
    }

    fn OnTestKeyDown(&self, _pic: &ITfContext, wParam: WPARAM, _lParam: LPARAM) -> HRESULT {
        let keycode = wParam.0 as u32;

        let (handled, _) = self.send_key_event(keycode);

        if handled {
            S_OK
        } else {
            S_FALSE
        }
    }

    fn OnTestKeyUp(&self, _pic: &ITfContext, _wParam: WPARAM, _lParam: LPARAM) -> HRESULT {
        S_FALSE
    }
}
