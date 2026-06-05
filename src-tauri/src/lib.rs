mod candidate_bar;
mod ime;
mod pipe_server;

use candidate_bar::BarData;
use ime::rime::{ContextData, KeyResult, RimeEngine, StatusData};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, State,
};

struct AppState {
    engine: RimeEngine,
    initialized: AtomicBool,
    bar_state: Arc<Mutex<BarData>>,
    bar_hwnd: Arc<Mutex<isize>>,
}

fn lock_bar<'a>(state: &'a AppState) -> Result<std::sync::MutexGuard<'a, BarData>, String> {
    state.bar_state.lock().map_err(|_| "Mutex poisoned".to_string())
}

fn lock_hwnd<'a>(state: &'a AppState) -> Result<std::sync::MutexGuard<'a, isize>, String> {
    state.bar_hwnd.lock().map_err(|_| "Mutex poisoned".to_string())
}

fn update_bar_from_context(state: &AppState, ctx: &ContextData) {
    if let Ok(mut bar) = lock_bar(state) {
        bar.preedit = ctx.preedit.clone();
        bar.candidates = ctx.candidates.iter().map(|c| c.text.clone()).collect();
        bar.active_index = ctx.highlighted_index as usize;
        bar.page_no = ctx.page_no as usize;
        bar.visible = true;
    }
    if let Ok(hwnd) = lock_hwnd(state) {
        if *hwnd != 0 {
            candidate_bar::signal_update(*hwnd);
        }
    }
}

fn hide_bar(state: &AppState) {
    if let Ok(mut bar) = lock_bar(state) {
        bar.visible = false;
        bar.preedit.clear();
        bar.candidates.clear();
    }
    if let Ok(hwnd) = lock_hwnd(state) {
        if *hwnd != 0 {
            candidate_bar::signal_update(*hwnd);
        }
    }
}

#[tauri::command]
fn init_ime(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    if state.initialized.load(Ordering::SeqCst) {
        return Ok(());
    }
    state.engine.initialize(&app)?;
    state.initialized.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
fn process_key(state: State<AppState>, keycode: i32, modifiers: i32) -> Result<Option<ContextData>, String> {
    match state.engine.process_key(keycode, modifiers)? {
        KeyResult::Handled(ctx) => {
            if !ctx.commit_text.is_empty() {
                hide_bar(&state);
            } else {
                update_bar_from_context(&state, &ctx);
            }
            Ok(Some(ctx))
        }
        KeyResult::NotHandled => Ok(None),
    }
}

#[tauri::command]
fn select_candidate(state: State<AppState>, index: i32) -> Result<Option<ContextData>, String> {
    let result = state.engine.select_candidate(index)?;

    if let Some(ref ctx) = result {
        if !ctx.commit_text.is_empty() {
            hide_bar(&state);
        } else {
            update_bar_from_context(&state, ctx);
        }
    }

    Ok(result)
}

#[tauri::command]
fn clear_composition(state: State<AppState>) -> Result<(), String> {
    state.engine.clear_composition()?;
    hide_bar(&state);
    Ok(())
}

#[tauri::command]
fn get_ime_status(state: State<AppState>) -> Result<StatusData, String> {
    state.engine.get_status()
}

#[tauri::command]
fn update_bar_theme(state: State<AppState>, theme: String) -> Result<(), String> {
    lock_bar(&state)?.theme = theme;
    if let Ok(hwnd) = lock_hwnd(&state) {
        if *hwnd != 0 {
            candidate_bar::signal_update(*hwnd);
        }
    }
    Ok(())
}

#[tauri::command]
fn update_bar_color(state: State<AppState>, r: u8, g: u8, b: u8) -> Result<(), String> {
    lock_bar(&state)?.primary_color = (r, g, b);
    if let Ok(hwnd) = lock_hwnd(&state) {
        if *hwnd != 0 {
            candidate_bar::signal_update(*hwnd);
        }
    }
    Ok(())
}

#[tauri::command]
fn update_bar_position(state: State<AppState>, x: i32, y: i32) -> Result<(), String> {
    let mut bar = lock_bar(&state)?;
    bar.pos_x = x;
    bar.pos_y = y;
    Ok(())
}

fn format_pipe_response(ctx: &ContextData) -> String {
    serde_json::json!({
        "handled": true,
        "commit": ctx.commit_text,
        "preedit": ctx.preedit,
        "candidates": ctx.candidates.iter().map(|c| &c.text).collect::<Vec<_>>(),
    }).to_string()
}

fn create_pipe_handler(handle: AppHandle) -> impl Fn(String) -> Option<String> {
    move |request: String| -> Option<String> {
        let msg: serde_json::Value = match serde_json::from_str(&request) {
            Ok(v) => v,
            Err(_) => return Some(r#"{"handled":false}"#.into()),
        };

        if msg.get("type").and_then(|t| t.as_str()) != Some("keydown") {
            return Some(r#"{"handled":false}"#.into());
        }

        let keycode = msg["keycode"].as_i64().unwrap_or(0) as i32;
        let modifiers = msg["modifiers"].as_i64().unwrap_or(0) as i32;

        let state: State<AppState> = handle.state();

        match state.engine.process_key(keycode, modifiers) {
            Ok(KeyResult::Handled(ctx)) => {
                if !ctx.commit_text.is_empty() {
                    hide_bar(&state);
                } else {
                    update_bar_from_context(&state, &ctx);
                }

                Some(format_pipe_response(&ctx))
            }
            Ok(KeyResult::NotHandled) => Some(r#"{"handled":false}"#.into()),
            Err(e) => {
                eprintln!("process_key error: {}", e);
                Some(r#"{"handled":false}"#.into())
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let bar_state = Arc::new(Mutex::new(BarData::default()));
    let bar_hwnd: Arc<Mutex<isize>> = Arc::new(Mutex::new(0));
    candidate_bar::start_bar(bar_state.clone(), bar_hwnd.clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let handle = app.handle().clone();

            let settings_item = MenuItemBuilder::with_id("open_settings", "偏好设置...")
                .build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "退出 Lexi")
                .build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&settings_item)
                .separator()
                .item(&quit)
                .build()?;

            let icon_bytes = include_bytes!("../icons/icon.png");
            let img = image::load_from_memory(icon_bytes)
                .expect("Failed to decode icon")
                .to_rgba8();
            let (w, h) = img.dimensions();
            let icon = tauri::image::Image::new_owned(img.into_raw(), w, h);

            let _tray = TrayIconBuilder::new()
                .icon(icon)
                .tooltip("Lexi 输入法")
                .menu(&menu)
                .on_menu_event(|app, event| {
                    if event.id().as_ref() == "open_settings" {
                        if let Some(w) = app.get_webview_window("settings") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    } else if event.id().as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .build(app)?;

            let handle2 = handle.clone();
            tauri::async_runtime::spawn(async move {
                let server = pipe_server::PipeServer::new();
                let handler = create_pipe_handler(handle2);
                if let Err(e) = server.start(handler).await {
                    eprintln!("Pipe server error: {}", e);
                }
            });

            Ok(())
        })
        .manage(AppState {
            engine: RimeEngine::new(),
            initialized: AtomicBool::new(false),
            bar_state,
            bar_hwnd,
        })
        .invoke_handler(tauri::generate_handler![
            init_ime,
            process_key,
            select_candidate,
            clear_composition,
            get_ime_status,
            update_bar_theme,
            update_bar_color,
            update_bar_position,
        ])
        .run(tauri::generate_context!())
        .expect("error while running lexi input method");
}
