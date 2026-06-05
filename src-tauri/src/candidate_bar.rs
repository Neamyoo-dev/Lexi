use std::sync::{Arc, Mutex};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject,
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, DIB_RGB_COLORS, HDC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
    GetWindowLongPtrW, PostMessageW, RegisterClassExW, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, UpdateLayeredWindow,
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, GWLP_USERDATA, HWND_TOPMOST,
    MSG, SW_HIDE, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
    SW_SHOWNA, ULW_ALPHA, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
};

use skia_safe::{
    BlurStyle, Color, Color4f, ColorType, Font, FontMgr, FontStyle,
    ImageInfo, MaskFilter, Paint, Point, Rect, surfaces,
    Typeface, AlphaType,
};

const RENDER_MSG: u32 = 0x8001;

#[derive(Clone)]
pub struct BarData {
    pub preedit: String,
    pub candidates: Vec<String>,
    pub active_index: usize,
    pub page_no: usize,
    pub total_pages: usize,
    pub visible: bool,
    pub pos_x: i32,
    pub pos_y: i32,
    pub theme: String,
    pub primary_color: (u8, u8, u8),
}

impl Default for BarData {
    fn default() -> Self {
        BarData {
            preedit: String::new(),
            candidates: Vec::new(),
            active_index: 0,
            page_no: 0,
            total_pages: 0,
            visible: false,
            pos_x: 100,
            pos_y: 300,
            theme: "light".into(),
            primary_color: (74, 108, 247),
        }
    }
}

struct BarContext {
    state: Arc<Mutex<BarData>>,
    typeface: Typeface,
}

pub fn start_bar(state: Arc<Mutex<BarData>>, hwnd_out: Arc<Mutex<isize>>) {
    std::thread::spawn(move || run_bar(state, hwnd_out));
}

fn run_bar(state: Arc<Mutex<BarData>>, hwnd_out: Arc<Mutex<isize>>) {
    let hinst = unsafe {
        windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap()
    };

    let cn = wide("LexiCandBarSkia");
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(bar_wndproc),
        hInstance: hinst.into(),
        lpszClassName: PCWSTR(cn.as_ptr()),
        ..Default::default()
    };
    unsafe { RegisterClassExW(&wc); }

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            PCWSTR(cn.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            CW_USEDEFAULT, CW_USEDEFAULT, 400, 80,
            None, None, hinst, None,
        )
    };
    let hwnd = match hwnd {
        Ok(h) => {
            *hwnd_out.lock().unwrap() = h.0 as isize;
            h
        }
        Err(_) => return,
    };

    let fm = FontMgr::default();
    let typeface = fm.match_family_style("Microsoft YaHei", FontStyle::default())
        .or_else(|| fm.match_family_style("Arial", FontStyle::default()))
        .expect("No font available");

    let ctx = Box::new(BarContext { state, typeface });
    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(ctx) as isize); }

    let mut msg = MSG::default();
    loop {
        let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if ret.0 <= 0 { break; }
        unsafe { DispatchMessageW(&msg); }
    }
}

unsafe extern "system" fn bar_wndproc(
    hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    if msg == RENDER_MSG {
        let raw = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
        if raw == 0 { return LRESULT(0); }

        let ctx: &BarContext = unsafe { &*(raw as *const BarContext) };
        let data = ctx.state.lock().unwrap().clone();

        if data.visible && !data.candidates.is_empty() {
            unsafe {
                render_frame(hwnd, &data, &ctx.typeface);
                ShowWindow(hwnd, SW_SHOWNA);
                SetWindowPos(hwnd, HWND_TOPMOST,
                    data.pos_x, data.pos_y - 32, 0, 0,
                    SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER);
            }
        } else {
            unsafe { ShowWindow(hwnd, SW_HIDE); }
        }
        return LRESULT(0);
    }

    if msg == windows::Win32::UI::WindowsAndMessaging::WM_DESTROY {
        let raw = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
        if raw != 0 { let _ = unsafe { Box::from_raw(raw as *mut BarContext) }; }
        unsafe { windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0); }
        return LRESULT(0);
    }

    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

unsafe fn render_frame(hwnd: HWND, data: &BarData, typeface: &Typeface) {
    let n = data.candidates.len().max(1);
    let w = (n as i32 * 68 + 60).max(200);
    let h = if data.preedit.is_empty() { 48 } else { 68 };

    let bi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: 0,
            ..Default::default()
        },
        bmiColors: [std::mem::zeroed(); 1],
    };

    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let dib = unsafe { CreateDIBSection(HDC::default(), &bi, DIB_RGB_COLORS, &mut bits, None, 0) };
    let dib = match dib {
        Ok(d) => d,
        Err(_) => return,
    };

    let dc = unsafe { CreateCompatibleDC(HDC::default()) };
    let old_bmp = unsafe { SelectObject(dc, dib) };

    draw_skia(w, h, data, typeface, bits);

    let bf = BLENDFUNCTION { BlendOp: 0, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: 1 };
    let mut pt = POINT::default();
    let mut sz = SIZE { cx: w, cy: h };
    let mut ps = POINT::default();
    unsafe {
        let _ = UpdateLayeredWindow(
            hwnd, HDC::default(), Some(&mut pt), Some(&mut sz),
            dc, Some(&mut ps), COLORREF::default(), Some(&bf), ULW_ALPHA,
        );
        SelectObject(dc, old_bmp);
        DeleteDC(dc);
        DeleteObject(dib);
    }
}

fn to_color4f(c: Color) -> Color4f {
    Color4f::new(
        c.r() as f32 / 255.0,
        c.g() as f32 / 255.0,
        c.b() as f32 / 255.0,
        c.a() as f32 / 255.0,
    )
}

fn draw_skia(w: i32, h: i32, data: &BarData, typeface: &Typeface, bits: *mut std::ffi::c_void) {
    if bits.is_null() { return; }

    let image_info = ImageInfo::new(
        (w, h),
        ColorType::BGRA8888,
        AlphaType::Premul,
        None,
    );

    let row_bytes = (w * 4) as usize;
    let total = row_bytes * h as usize;
    let dst_slice = unsafe { std::slice::from_raw_parts_mut(bits as *mut u8, total) };

    let mut surface = surfaces::wrap_pixels(
        &image_info,
        dst_slice,
        row_bytes,
        None,
    ).expect("Failed to create Skia surface");

    let canvas = surface.canvas();
    canvas.clear(Color::TRANSPARENT);

    let is_dark = data.theme == "dark";
    let (pr, pg, pb) = data.primary_color;
    let accent = Color::from_argb(255, pr, pg, pb);
    let accent_bg = Color::from_argb(26, pr, pg, pb);

    draw_background(canvas, w, h, is_dark);
    draw_candidates_skia(canvas, w, h, data, typeface, accent, accent_bg, is_dark);
}

fn draw_background(canvas: &skia_safe::Canvas, w: i32, h: i32, is_dark: bool) {
    let r = 12.0;
    let rect = Rect::from_xywh(1.0, 1.0, (w - 2) as f32, (h - 2) as f32);

    let shadow_c = if is_dark {
        Color::from_argb(80, 0, 0, 0)
    } else {
        Color::from_argb(25, 0, 0, 0)
    };

    let mut shadow_paint = Paint::new(to_color4f(shadow_c), None);
    shadow_paint.set_anti_alias(true);
    shadow_paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 6.0, None));
    canvas.draw_round_rect(&rect, r, r, &shadow_paint);

    let (bg_top, _bg_bot) = if is_dark {
        ((28, 28, 33, 240), (20, 20, 24, 225))
    } else {
        ((255, 255, 255, 235), (245, 245, 250, 215))
    };
    let bg = Color::from_argb(bg_top.3, bg_top.0, bg_top.1, bg_top.2);

    let mut bg_paint = Paint::new(to_color4f(bg), None);
    bg_paint.set_anti_alias(true);
    canvas.draw_round_rect(&rect, r, r, &bg_paint);
}

fn draw_candidates_skia(
    canvas: &skia_safe::Canvas,
    w: i32,
    _h: i32,
    data: &BarData,
    typeface: &Typeface,
    accent: Color,
    accent_bg: Color,
    is_dark: bool,
) {
    let font_large = Font::from_typeface(typeface.clone(), 18.0);
    let font_small = Font::from_typeface(typeface.clone(), 14.0);
    let font_index = Font::from_typeface(typeface.clone(), 11.0);

    let text_color = if is_dark {
        Color::from_argb(255, 224, 224, 224)
    } else {
        Color::from_argb(255, 34, 34, 34)
    };
    let idx_color = if is_dark {
        Color::from_argb(255, 128, 128, 128)
    } else {
        Color::from_argb(255, 160, 160, 160)
    };
    let page_color = if is_dark {
        Color::from_argb(255, 96, 96, 96)
    } else {
        Color::from_argb(255, 153, 153, 153)
    };

    let y_start = if data.preedit.is_empty() { 14.0 } else { 34.0 };

    for (i, cand) in data.candidates.iter().enumerate() {
        let x = 16.0 + i as f32 * 68.0;
        let active = i == data.active_index;

        if active {
            let mut bg_paint = Paint::new(to_color4f(accent_bg), None);
            bg_paint.set_anti_alias(true);
            let bg_rect = Rect::from_xywh(x - 2.0, y_start - 2.0, 60.0, 24.0);
            canvas.draw_round_rect(&bg_rect, 6.0, 6.0, &bg_paint);
        }

        let idx_text = (i + 1).to_string();
        let mut idx_paint = Paint::new(to_color4f(if active { accent } else { idx_color }), None);
        idx_paint.set_anti_alias(true);
        canvas.draw_str(&idx_text, (x + 2.0, y_start + 13.0), &font_index, &idx_paint);

        let mut txt_paint = Paint::new(to_color4f(if active { accent } else { text_color }), None);
        txt_paint.set_anti_alias(true);
        canvas.draw_str(cand, (x + 22.0, y_start + 15.0), &font_large, &txt_paint);
    }

    if data.total_pages > 1 {
        let page_str = format!("{}/{}", data.page_no + 1, data.total_pages);
        let mut paint = Paint::new(to_color4f(page_color), None);
        paint.set_anti_alias(true);
        canvas.draw_str(&page_str, ((w - 48) as f32, (_h - 6) as f32), &font_small, &paint);
    }
}

fn wide(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    v
}

pub fn signal_update(hwnd: isize) {
    if hwnd == 0 { return; }
    unsafe { PostMessageW(HWND(hwnd as *mut _), RENDER_MSG, WPARAM(0), LPARAM(0)); }
}
