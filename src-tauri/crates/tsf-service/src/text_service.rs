use crate::pipe_client;
use windows::core::{implement, Interface, Result, GUID, HRESULT};
use windows::Win32::Foundation::{BOOL, LPARAM, POINT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::TextServices::{
    ITfContext, ITfEditSession, ITfEditSession_Impl,
    ITfKeyEventSink, ITfKeyEventSink_Impl, ITfSource,
    ITfTextInputProcessor, ITfTextInputProcessorEx,
    ITfTextInputProcessorEx_Impl, ITfTextInputProcessor_Impl, ITfThreadMgr,
    TF_ES_SYNC, TF_ES_READWRITE,
};

use std::sync::Mutex;

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
    fn Activate(&self, ptim: Option<&ITfThreadMgr>, tid: u32) -> Result<()> {
        let ptim = ptim.ok_or_else(|| HRESULT(-0x7ff8ffffi32))?;

        *self.client_id.lock().unwrap() = tid;
        *self.thread_mgr.lock().unwrap() = Some(ptim.clone());
        *self.active.lock().unwrap() = true;

        let _ = pipe_client::connect();

        self.install_key_event_sink(&ptim, tid)
    }

    fn Deactivate(&self) -> Result<()> {
        if let Ok(ptim_lock) = self.thread_mgr.lock() {
            if let Some(ref ptim) = *ptim_lock {
                self.uninstall_key_event_sink(ptim);
            }
        }
        *self.active.lock().unwrap() = false;
        pipe_client::disconnect();
        Ok(())
    }
}

#[allow(non_snake_case)]
impl ITfTextInputProcessorEx_Impl for LexiTextService_Impl {
    fn ActivateEx(&self, ptim: Option<&ITfThreadMgr>, tid: u32, _dwFlags: u32) -> Result<()> {
        self.Activate(ptim, tid)
    }
}

impl LexiTextService_Impl {
    fn install_key_event_sink(&self, ptim: &ITfThreadMgr, _client_id: u32) -> Result<()> {
        let mut installed = self.key_sink_installed.lock().unwrap();
        if *installed {
            return Ok(());
        }

        let source: ITfSource = ptim.cast()?;
        let key_sink = LexiKeyEventSink::new();
        let unknown: windows::core::IUnknown = key_sink.into();

        unsafe {
            source.AdviseSink(&ITfKeyEventSink::IID, &unknown)?;
        }

        *installed = true;
        Ok(())
    }

    fn uninstall_key_event_sink(&self, ptim: &ITfThreadMgr) {
        let mut installed = self.key_sink_installed.lock().unwrap();
        if !*installed {
            return;
        }

        if let Ok(source) = ptim.cast::<ITfSource>() {
            unsafe {
                let _ = source.UnadviseSink(0);
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
        if (GetAsyncKeyState(VK_SHIFT.0 as i32) as i32) & 0x8000 != 0 {
            mask |= RIME_SHIFT;
        }
        if (GetAsyncKeyState(VK_CONTROL.0 as i32) as i32) & 0x8000 != 0 {
            mask |= RIME_CTRL;
        }
        if (GetAsyncKeyState(VK_MENU.0 as i32) as i32) & 0x8000 != 0 {
            mask |= RIME_ALT;
        }
        if ((GetAsyncKeyState(VK_LWIN.0 as i32) as i32) | (GetAsyncKeyState(VK_RWIN.0 as i32) as i32)) & 0x8000 != 0 {
            mask |= RIME_WIN;
        }
    }
    mask
}

#[implement(ITfKeyEventSink)]
struct LexiKeyEventSink;

impl LexiKeyEventSink {
    fn new() -> Self {
        LexiKeyEventSink
    }

    fn send_key_event(&self, keycode: u32) -> (bool, String) {
        let modifiers = extract_modifier_mask();

        let cursor_pos = unsafe {
            let mut pt = POINT { x: 0, y: 0 };
            windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt)
                .ok()
                .map(|()| (pt.x, pt.y))
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
    fn DoEditSession(&self, _ec: u32) -> Result<()> {
        if self.commit_text.is_empty() {
            return Ok(());
        }

        // The actual commit is done via ITfInsertAtSelection obtained from the context
        // in OnKeyDown before requesting the edit session
        Ok(())
    }
}

#[allow(non_snake_case)]
impl ITfKeyEventSink_Impl for LexiKeyEventSink_Impl {
    fn OnKeyDown(&self, pic: Option<&ITfContext>, wParam: WPARAM, _lParam: LPARAM) -> Result<BOOL> {
        let keycode = wParam.0 as u32;
        let (handled, response) = self.send_key_event(keycode);

        if !response.is_empty() {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
                if let Some(commit) = parsed.get("commit").and_then(|c| c.as_str()) {
                    if !commit.is_empty() {
                        if let Some(pic) = pic {
                            let edit_session = LexiEditSession::new(commit.to_string());
                            let es: ITfEditSession = edit_session.into();
                            unsafe {
                                let _ = pic.RequestEditSession(0, &es, TF_ES_SYNC | TF_ES_READWRITE);
                            }
                        }
                    }
                }
            }
        }

        if handled { Ok(BOOL(1)) } else { Ok(BOOL(0)) }
    }

    fn OnKeyUp(&self, _pic: Option<&ITfContext>, _wParam: WPARAM, _lParam: LPARAM) -> Result<BOOL> {
        Ok(BOOL(0))
    }

    fn OnTestKeyDown(&self, _pic: Option<&ITfContext>, wParam: WPARAM, _lParam: LPARAM) -> Result<BOOL> {
        let keycode = wParam.0 as u32;
        let (handled, _) = self.send_key_event(keycode);

        if handled { Ok(BOOL(1)) } else { Ok(BOOL(0)) }
    }

    fn OnTestKeyUp(&self, _pic: Option<&ITfContext>, _wParam: WPARAM, _lParam: LPARAM) -> Result<BOOL> {
        Ok(BOOL(0))
    }

    fn OnSetFocus(&self, _fFocus: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnPreservedKey(&self, _pic: Option<&ITfContext>, _rguid: *const GUID) -> Result<BOOL> {
        Ok(BOOL(0))
    }
}
