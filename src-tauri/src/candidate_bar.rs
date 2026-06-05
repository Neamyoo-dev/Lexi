use skia_safe::{Canvas, Color, Font, FontMgr, FontStyle, Paint, PaintStyle, Rect, Surface};
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, EndPaint,
    GetDC, GetDesktopWindow, GetDeviceCaps, GetWindowDC, ReleaseDC, SelectObject,
    UpdateLayeredWindow, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    PAINTSTRUCT, RGBQUAD,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetMessageW, LoadCursorW,
    PostMessageW, RegisterClassW, SetWindowPos, TranslateMessage, CS_HREDRAW, CS_VREDRAW,
    CW_USEDEFAULT, HMENU, HWND_BOTTOM, HWND_TOPMOST, IDC_ARROW,
    MSG, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_CREATE, WM_DESTROY, WM_PAINT, WS_CAPTION, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP, WS_SYSMENU,
};

const RENDER_MSG: u32 = WM_PAINT + 100;
const CANVAS_WIDTH: i32 = 680;
const CANVAS_HEIGHT: i32 = 52;
const CANDIDATE_WIDTH: f32 = 68.0;
const CANDIDATE_GAP: f32 = 4.0;
const BAR_PADDING: f32 = 12.0;
const CORNER_RADIUS: f32 = 10.0;
const PREEDIT_HEIGHT: f32 = 20.0;

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
            primary_color: (70, 130, 180),
        }
    }
}

struct BarContext {
    state: Arc<Mutex<BarData>>,
    hwnd: isize,
    typeface: skia_safe::Typeface,
}

fn get_bar_size(data: &BarData) -> (f32, f32) {
    let count = data.candidates.len().max(1) as f32;
    let w = count * CANDIDATE_WIDTH + (count - 1.0) * CANDIDATE_GAP + BAR_PADDING * 2.0;
    let h = if data.preedit.is_empty() {
        CANVAS_HEIGHT as f32
    } else {
        CANVAS_HEIGHT as f32 + PREEDIT_HEIGHT
    };
    (w, h)
}

pub fn start_bar(state: Arc<Mutex<BarData>>, hwnd_out: Arc<Mutex<isize>>) {
    std::thread::spawn(move || {
        run_bar(state, hwnd_out);
    });
}

fn run_bar(state: Arc<Mutex<BarData>>, hwnd_out: Arc<Mutex<isize>>) {
    let fm = FontMgr::default();
    let typeface = fm
        .match_family_style("Microsoft YaHei", FontStyle::default())
        .or_else(|| fm.match_family_style("Arial", FontStyle::default()))
        .or_else(|| fm.default_family_style())
        .expect("No font available");

    let hinstance = unsafe {
        windows::Win32::System::LibraryLoader::GetModuleHandleA(None)
    };

    let class_name = windows::core::w!("LexiCandidateBar");

    let wc = windows::Win32::UI::WindowsAndMessaging::WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(bar_wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: None,
        hCursor: Some(unsafe { LoadCursorW(None, IDC_ARROW) }),
        hbrBackground: None,
        lpszMenuName: None,
        lpszClassName: class_name,
    };

    let atom = unsafe { RegisterClassW(&wc) };
    if atom == 0 {
        return;
    }

    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW),
            class_name,
            windows::core::w!(""),
            WINDOW_STYLE(WS_POPUP),
            100,
            300,
            200,
            50,
            None,
            HMENU(std::ptr::null_mut()),
            hinstance,
            None,
        )
    };

    if hwnd.is_invalid() {
        return;
    }

    {
        let mut hwnd_guard = hwnd_out.lock().unwrap();
        *hwnd_guard = hwnd.0 as isize;
    }

    let ctx = BarContext {
        state,
        hwnd: hwnd.0 as isize,
        typeface,
    };

    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
    }

    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            let _ = DefWindowProcW(msg.hwnd, msg.message, msg.wParam, msg.lParam);
        }
    }
}

extern "system" fn bar_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let create_struct = unsafe { &*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW) };
            LRESULT(0)
        }
        RENDER_MSG => {
            let ptr = unsafe {
                let p = windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, 0);
                if p == 0 {
                    return LRESULT(0);
                }
                p as *mut BarContext
            };
            let ctx = unsafe { &mut *ptr };
            render_frame(hwnd, ctx);
            LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = unsafe {
                let p = windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, 0);
                p as *mut BarContext
            };
            if !ptr.is_null() {
                let _ = unsafe { Box::from_raw(ptr) };
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn render_frame(hwnd: HWND, ctx: &BarContext) {
    let snapshot = {
        let guard = ctx.state.lock().unwrap();
        (*guard).clone()
    };

    let is_dark = snapshot.theme == "dark";
    let (w, h) = get_bar_size(&snapshot);
    let w = w as i32;
    let h = h as i32;

    let hdc = unsafe { GetDC(None) };
    if hdc.is_invalid() {
        return;
    }

    let dib_size = w * h * 4;
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [RGBQUAD::default(); 1],
    };

    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let dib = unsafe { CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) };
    if dib.is_invalid() || bits.is_null() {
        unsafe { let _ = ReleaseDC(None, hdc); }
        return;
    }

    let mdc = unsafe { CreateCompatibleDC(hdc) };
    if mdc.is_invalid() {
        unsafe {
            let _ = ReleaseDC(None, hdc);
            let _ = DeleteObject(dib);
        }
        return;
    }

    let old_bmp = unsafe { SelectObject(mdc, dib) };

    let bits_len = dib_size as usize;
    let bytes = unsafe { std::slice::from_raw_parts_mut(bits as *mut u8, bits_len) };

    draw_skia(w, h, &snapshot, &ctx.typeface, bytes);

    let mut blend = windows::Win32::Graphics::Gdi::BLENDFUNCTION {
        BlendOp: 0,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: 1,
    };

    unsafe {
        let _ = UpdateLayeredWindow(
            hwnd,
            hdc,
            None,
            None,
            Some(mdc),
            Some(&POINT { x: 0, y: 0 }),
            0,
            Some(&blend),
            windows::Win32::Graphics::Gdi::ULW_ALPHA,
        );
    }

    unsafe {
        let _ = SelectObject(mdc, old_bmp);
        let _ = DeleteDC(mdc);
        let _ = DeleteObject(dib);
        let _ = ReleaseDC(None, hdc);
    }
}

struct RenderCache {
    candidate_count: usize,
    has_preedit: bool,
    w: i32,
    h: i32,
    font_large: Font,
    font_small: Font,
    font_index: Font,
}

fn get_or_create_fonts(typeface: &skia_safe::Typeface) -> (Font, Font, Font) {
    (
        Font::from_typeface(typeface.clone(), 18.0),
        Font::from_typeface(typeface.clone(), 14.0),
        Font::from_typeface(typeface.clone(), 11.0),
    )
}

fn draw_skia(
    w: i32,
    h: i32,
    data: &BarData,
    typeface: &skia_safe::Typeface,
    pixels: &mut [u8],
) {
    let mut surface = unsafe {
        Surface::new_legacy_render_target(
            pixels.as_mut_ptr(),
            w as i32,
            h as i32,
            w as i32 * 4,
            None,
        )
    };

    let canvas = surface.canvas();
    canvas.clear(Color::TRANSPARENT);

    let is_dark = data.theme == "dark";
    let (r, g, b) = data.primary_color;
    let primary = Color::from_rgb(r, g, b);

    let bg_color = if is_dark {
        Color::from_argb(220, 30, 30, 30)
    } else {
        Color::from_argb(220, 245, 245, 245)
    };

    let text_color = if is_dark {
        Color::from_rgb(240, 240, 240)
    } else {
        Color::from_rgb(40, 40, 40)
    };

    let active_bg = Color::from_argb(60, r, g, b);
    let preedit_offset = if data.preedit.is_empty() {
        0.0
    } else {
        PREEDIT_HEIGHT
    };

    let rect = Rect::new(0.0, 0.0, w as f32, h as f32);
    draw_background(canvas, rect, bg_color);

    draw_candidates(
        canvas,
        data,
        primary,
        text_color,
        bg_color,
        active_bg,
        typeface,
        preedit_offset,
    );
}

fn draw_background(canvas: &Canvas, rect: Rect, bg_color: Color) {
    let mut paint = Paint::new(bg_color, None);
    paint.set_anti_alias(true);
    let rrect = skia_safe::RRect::new_rect_radii(
        rect,
        &[CORNER_RADIUS; 4].into(),
    );
    canvas.draw_rrect(rrect, &paint);
}

fn draw_candidates(
    canvas: &Canvas,
    data: &BarData,
    primary: Color,
    text_color: Color,
    bg_color: Color,
    active_bg: Color,
    typeface: &skia_safe::Typeface,
    preedit_offset: f32,
) {
    let (font_large, font_small, font_index) = get_or_create_fonts(typeface);

    let candidate_count = data.candidates.len().max(1);
    let bar_width = candidate_count as f32 * CANDIDATE_WIDTH
        + (candidate_count as f32 - 1.0) * CANDIDATE_GAP
        + BAR_PADDING * 2.0;

    let y_center = CANVAS_HEIGHT as f32 / 2.0 + 6.0 + preedit_offset;

    for (i, candidate) in data.candidates.iter().enumerate() {
        let x = BAR_PADDING + i as f32 * (CANDIDATE_WIDTH + CANDIDATE_GAP);

        if i == data.active_index {
            let mut highlight = Paint::new(active_bg, None);
            highlight.set_anti_alias(true);
            let hr = skia_safe::RRect::new_rect_radii(
                Rect::new(x, y_center - 10.0, x + CANDIDATE_WIDTH, y_center + 14.0),
                &[6.0; 4].into(),
            );
            canvas.draw_rrect(hr, &highlight);
        }

        let index_text = format!("{}", i + 1);
        canvas.draw_str(
            &index_text,
            x + 4.0,
            y_center,
            &font_index,
            &Paint::new(primary, None),
        );

        canvas.draw_str(
            candidate,
            x + 22.0,
            y_center + 0.5,
            &font_large,
            &Paint::new(text_color, None),
        );
    }
}

pub fn signal_update(hwnd: isize) {
    if hwnd != 0 {
        unsafe {
            let _ = PostMessageW(HWND(hwnd as _), RENDER_MSG, WPARAM(0), LPARAM(0));
        }
    }
}
