use crate::pipe_client;
use windows::core::{implement, ComObject, GUID, HRESULT, PCWSTR};
use windows::Win32::Foundation::{BOOL, FALSE, HWND, LPARAM, LRESULT, POINT, RECT, TRUE, WPARAM};
use windows::Win32::UI::TextServices::{
    IEnumTfContexts, IEnumTfDisplayAttributeInfo, IEnumTfFunctionProviders,
    IEnumTfInputProcessorProfiles, ITfCandidateListUIElement,
    ITfClientId, ITfCompartment, ITfCompartmentMgr,
    ITfContext, ITfContextInputEnd, ITfContextKeyEventSink,
    ITfDisplayAttributeMgr, ITfDocumentMgr, ITfEditSession, ITfInputProcessorProfileActivationSink,
    ITfKeyEventSink, ITfKeyTraceEventSink, ITfMessagePump,
    ITfReadingInformationUIElement, ITfSource, ITfSourceSingle,
    ITfTextInputProcessor, ITfTextInputProcessorEx,
    ITfThreadMgr, ITfThreadMgrEventSink,
    TF_DISPLAYATTRIBUTE, TF_INPUTPROCESSOR_PROFILE,
    TF_IPSINK_FLAG, TF_SAS_CONTROL, TF_SD_READONLY, TF_SS_HASINPUT,
};

use std::sync::Mutex;

const LEXI_KEYBOARD_ID: &str = "Lexi";

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
    fn Activate(
        &self,
        ptim: &ITfThreadMgr,
        tid: &ITfClientId,
    ) -> HRESULT {
        let client_id = unsafe { tid.GetClientId() };

        *self.client_id.lock().unwrap() = client_id;
        *self.thread_mgr.lock().unwrap() = Some(ptim.clone());
        *self.active.lock().unwrap() = true;

        if let Ok(h) = pipe_client::connect() {
            let _ = h;
        }

        self.install_key_event_sink(ptim, client_id)
    }

    fn Deactivate(&self, ptim: &ITfThreadMgr, tid: &ITfClientId) -> HRESULT {
        self.uninstall_key_event_sink(ptim);
        *self.active.lock().unwrap() = false;
        pipe_client::disconnect();
        HRESULT(0)
    }
}

#[allow(non_snake_case)]
impl ITfTextInputProcessorEx_Impl for LexiTextService_Impl {
    fn ActivateEx(
        &self,
        ptim: &ITfThreadMgr,
        tid: &ITfClientId,
        _dwFlags: u32,
    ) -> HRESULT {
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
            let key_sink = LexiKeyEventSink::new();
            let unknown: windows::core::IUnknown = key_sink.into();

            let cookie = unsafe {
                source.AdviseSink(
                    &ITfKeyEventSink::IID,
                    &unknown,
                )
            };

            if cookie.is_ok() {
                *installed = true;
            }
            std::mem::forget(unknown);
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

#[implement(ITfKeyEventSink)]
struct LexiKeyEventSink {
    _private: (),
}

impl LexiKeyEventSink {
    fn new() -> Self {
        LexiKeyEventSink { _private: () }
    }
}

#[allow(non_snake_case)]
impl ITfKeyEventSink_Impl for LexiKeyEventSink_Impl {
    fn OnKeyDown(
        &self,
        pic: &ITfContext,
        wParam: WPARAM,
        lParam: LPARAM,
    ) -> HRESULT {
        let keycode = wParam.0 as u32;

        let msg = format!(r#"{{"type":"keydown","keycode":{},"modifiers":0}}"#, keycode);

        if let Ok(Some(response)) = pipe_client::send_message(&msg) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
                if parsed.get("commit").and_then(|c| c.as_str()).map(|s| !s.is_empty()).unwrap_or(false) {
                    return HRESULT(0);
                }
            }
        }

        HRESULT(0)
    }

    fn OnKeyUp(
        &self,
        _pic: &ITfContext,
        _wParam: WPARAM,
        _lParam: LPARAM,
    ) -> HRESULT {
        HRESULT(0)
    }

    fn OnTestKeyDown(
        &self,
        _pic: &ITfContext,
        _wParam: WPARAM,
        _lParam: LPARAM,
    ) -> HRESULT {
        HRESULT(0)
    }

    fn OnTestKeyUp(
        &self,
        _pic: &ITfContext,
        _wParam: WPARAM,
        _lParam: LPARAM,
    ) -> HRESULT {
        HRESULT(0)
    }
}
