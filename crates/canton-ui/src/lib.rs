use canton_core::CandidatePage;
use std::cell::RefCell;
use std::rc::Rc;
use windows::core::{w, Interface, PCWSTR};
use windows::Win32::Foundation::{
    D2DERR_RECREATE_TARGET, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F, D2D_SIZE_U};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1RenderTarget,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_HWND_RENDER_TARGET_PROPERTIES,
    D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetMonitorInfoW, MonitorFromPoint, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, RegisterClassExW,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, CREATESTRUCTW, CS_DROPSHADOW, CS_HREDRAW,
    CS_IME, CS_VREDRAW, GWLP_USERDATA, HWND_TOPMOST, MA_NOACTIVATE, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, SW_HIDE, WM_CREATE, WM_DESTROY, WM_ERASEBKGND, WM_MOUSEACTIVATE, WM_PAINT,
    WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

const CLASS_NAME: PCWSTR = w!("CantonHKCandidateWindowClass");

struct ThemeColors {
    bg: D2D1_COLOR_F,
    border: D2D1_COLOR_F,
    text: D2D1_COLOR_F,
    muted: D2D1_COLOR_F,
    accent_bg: D2D1_COLOR_F,
    accent_text: D2D1_COLOR_F,
    header_bg: D2D1_COLOR_F,
}

impl ThemeColors {
    fn from_system() -> Self {
        if is_system_light_theme() {
            Self {
                bg: rgba(0xF9, 0xF9, 0xF9, 1.0),
                border: rgba(0xD8, 0xD8, 0xD8, 1.0),
                text: rgba(0x1A, 0x1A, 0x1A, 1.0),
                muted: rgba(0x66, 0x66, 0x66, 1.0),
                accent_bg: rgba(0x00, 0x5F, 0xB8, 0.12),
                accent_text: rgba(0x00, 0x5F, 0xB8, 1.0),
                header_bg: rgba(0xF0, 0xF2, 0xF5, 1.0),
            }
        } else {
            Self {
                bg: rgba(0x20, 0x20, 0x20, 1.0),
                border: rgba(0x3D, 0x3D, 0x3D, 1.0),
                text: rgba(0xF3, 0xF3, 0xF3, 1.0),
                muted: rgba(0xA0, 0xA0, 0xA0, 1.0),
                accent_bg: rgba(0x60, 0xCD, 0xFF, 0.18),
                accent_text: rgba(0x60, 0xCD, 0xFF, 1.0),
                header_bg: rgba(0x2B, 0x2B, 0x2B, 1.0),
            }
        }
    }
}

const fn rgba(r: u8, g: u8, b: u8, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: (r as f32) / 255.0,
        g: (g as f32) / 255.0,
        b: (b as f32) / 255.0,
        a,
    }
}

fn is_system_light_theme() -> bool {
    unsafe {
        let mut hkey = Default::default();
        let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey, 0, KEY_READ, &mut hkey).is_ok() {
            let mut val: u32 = 1;
            let mut size = std::mem::size_of::<u32>() as u32;
            let ok = RegQueryValueExW(
                hkey,
                w!("AppsUseLightTheme"),
                None,
                None,
                Some(&mut val as *mut u32 as *mut u8),
                Some(&mut size),
            )
            .is_ok();
            let _ = RegCloseKey(hkey);
            if ok {
                return val != 0;
            }
        }
    }
    true
}

struct WindowInner {
    hwnd: HWND,
    d2d_factory: ID2D1Factory,
    _dwrite_factory: IDWriteFactory,
    render_target: Option<ID2D1HwndRenderTarget>,
    text_format_cand: IDWriteTextFormat,
    text_format_header: IDWriteTextFormat,
    text_format_badge: IDWriteTextFormat,
    page: Option<CandidatePage>,
    visible: bool,
}

pub struct CandidateWindow {
    inner: Rc<RefCell<WindowInner>>,
}

impl CandidateWindow {
    pub fn new() -> windows::core::Result<Self> {
        unsafe {
            let hinstance = GetModuleHandleW(None)?;
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW | CS_IME | CS_DROPSHADOW,
                lpfnWndProc: Some(wnd_proc),
                hInstance: hinstance.into(),
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            let _ = RegisterClassExW(&wc);

            let d2d_factory: ID2D1Factory =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dwrite_factory: IDWriteFactory =
                DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;

            let text_format_cand = dwrite_factory.CreateTextFormat(
                w!("Microsoft JhengHei UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                15.0,
                w!("zh-TW"),
            )?;
            let _ = text_format_cand.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
            let _ = text_format_cand.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);

            let text_format_header = dwrite_factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                13.0,
                w!("en-US"),
            )?;
            let _ = text_format_header.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);

            let text_format_badge = dwrite_factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                12.0,
                w!("en-US"),
            )?;
            let _ = text_format_badge.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
            let _ = text_format_badge.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_TRAILING);

            let inner = Rc::new(RefCell::new(WindowInner {
                hwnd: HWND::default(),
                d2d_factory,
                _dwrite_factory: dwrite_factory,
                render_target: None,
                text_format_cand,
                text_format_header,
                text_format_badge,
                page: None,
                visible: false,
            }));

            let raw_ptr = Rc::into_raw(inner.clone()) as *const std::ffi::c_void;
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                CLASS_NAME,
                w!("CantonHK Candidates"),
                WS_POPUP,
                0,
                0,
                220,
                320,
                None,
                None,
                hinstance,
                Some(raw_ptr),
            )?;

            // Enable Windows 11 rounded corners when supported (ignored on Win10)
            let corner_pref = DWMWCP_ROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner_pref as *const _ as *const std::ffi::c_void,
                std::mem::size_of_val(&corner_pref) as u32,
            );

            inner.borrow_mut().hwnd = hwnd;

            Ok(Self { inner })
        }
    }

    pub fn show_page(
        &mut self,
        page: &CandidatePage,
        anchor_x: i32,
        anchor_y: i32,
        line_height: i32,
    ) {
        let mut inner = self.inner.borrow_mut();
        let hwnd = inner.hwnd;
        inner.page = Some(page.clone());
        inner.visible = true;

        unsafe {
            let dpi = GetDpiForWindow(hwnd).max(96) as f32;
            let scale = dpi / 96.0;

            let rows = page.items.len().max(1) as f32;
            let width_dip = 240.0_f32;
            let header_h_dip = 32.0_f32;
            let row_h_dip = 30.0_f32;
            let pad_bottom_dip = 6.0_f32;
            let height_dip = header_h_dip + rows * row_h_dip + pad_bottom_dip;

            let width_px = (width_dip * scale).ceil() as i32;
            let height_px = (height_dip * scale).ceil() as i32;

            // Clamp popup position to the active monitor's work area
            let pt = POINT {
                x: anchor_x,
                y: anchor_y,
            };
            let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let mut pos_x = anchor_x;
            let mut pos_y = anchor_y + line_height.max(4);
            if GetMonitorInfoW(hmon, &mut mi).as_bool() {
                let work = mi.rcWork;
                if pos_x + width_px > work.right {
                    pos_x = (work.right - width_px).max(work.left);
                }
                if pos_x < work.left {
                    pos_x = work.left;
                }
                if pos_y + height_px > work.bottom {
                    // Flip above the caret if it overflows below
                    let above_y = anchor_y - height_px - 4;
                    if above_y >= work.top {
                        pos_y = above_y;
                    } else {
                        pos_y = (work.bottom - height_px).max(work.top);
                    }
                }
            }

            if let Some(rt) = inner.render_target.as_ref() {
                let _ = rt.Resize(&D2D_SIZE_U {
                    width: width_px as u32,
                    height: height_px as u32,
                });
            }

            let _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                pos_x,
                pos_y,
                width_px,
                height_px,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(hwnd, None, false);
        }
    }

    pub fn hide(&mut self) {
        let mut inner = self.inner.borrow_mut();
        if inner.visible {
            inner.visible = false;
            inner.page = None;
            unsafe {
                let _ = ShowWindow(inner.hwnd, SW_HIDE);
            }
        }
    }

    pub fn hwnd(&self) -> HWND {
        self.inner.borrow().hwnd
    }
}

impl Drop for CandidateWindow {
    fn drop(&mut self) {
        let hwnd = self.inner.borrow().hwnd;
        if !hwnd.is_invalid() {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as _);
            LRESULT(0)
        }
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<WindowInner>;
            let mut ps = PAINTSTRUCT::default();
            BeginPaint(hwnd, &mut ps);
            if !ptr.is_null() {
                let inner_ref = &*ptr;
                if let Ok(mut inner) = inner_ref.try_borrow_mut() {
                    paint_window(hwnd, &mut inner);
                }
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<WindowInner>;
            if !ptr.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                let _ = Rc::from_raw(ptr);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn ensure_render_target(
    hwnd: HWND,
    inner: &mut WindowInner,
) -> windows::core::Result<ID2D1HwndRenderTarget> {
    if let Some(rt) = inner.render_target.as_ref() {
        return Ok(rt.clone());
    }
    let mut rc = RECT::default();
    windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rc)?;
    let size = D2D_SIZE_U {
        width: (rc.right - rc.left).max(1) as u32,
        height: (rc.bottom - rc.top).max(1) as u32,
    };
    let dpi = GetDpiForWindow(hwnd).max(96) as f32;
    let mut rt_props = D2D1_RENDER_TARGET_PROPERTIES::default();
    rt_props.dpiX = dpi;
    rt_props.dpiY = dpi;
    let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
        hwnd,
        pixelSize: size,
        presentOptions: D2D1_PRESENT_OPTIONS_NONE,
    };
    let rt = inner
        .d2d_factory
        .CreateHwndRenderTarget(&rt_props, &hwnd_props)?;
    inner.render_target = Some(rt.clone());
    Ok(rt)
}

unsafe fn paint_window(hwnd: HWND, inner: &mut WindowInner) {
    let Ok(hwnd_rt) = ensure_render_target(hwnd, inner) else {
        return;
    };
    let Ok(rt) = hwnd_rt.cast::<ID2D1RenderTarget>() else {
        return;
    };
    let Some(page) = inner.page.clone() else {
        return;
    };

    let theme = ThemeColors::from_system();
    let size = rt.GetSize();

    rt.BeginDraw();
    rt.Clear(Some(&theme.bg));

    let Ok(brush_header) = rt.CreateSolidColorBrush(&theme.header_bg, None) else {
        let _ = rt.EndDraw(None, None);
        return;
    };
    let Ok(brush_border) = rt.CreateSolidColorBrush(&theme.border, None) else {
        let _ = rt.EndDraw(None, None);
        return;
    };
    let Ok(brush_text) = rt.CreateSolidColorBrush(&theme.text, None) else {
        let _ = rt.EndDraw(None, None);
        return;
    };
    let Ok(brush_muted) = rt.CreateSolidColorBrush(&theme.muted, None) else {
        let _ = rt.EndDraw(None, None);
        return;
    };
    let Ok(brush_accent_bg) = rt.CreateSolidColorBrush(&theme.accent_bg, None) else {
        let _ = rt.EndDraw(None, None);
        return;
    };
    let Ok(brush_accent_text) = rt.CreateSolidColorBrush(&theme.accent_text, None) else {
        let _ = rt.EndDraw(None, None);
        return;
    };

    // Header bar
    let header_rect = D2D_RECT_F {
        left: 0.0,
        top: 0.0,
        right: size.width,
        bottom: 32.0,
    };
    rt.FillRectangle(&header_rect, &brush_header);

    // Pre-edit text on left of header
    let preedit_w: Vec<u16> = page.preedit.encode_utf16().collect();
    let preedit_rect = D2D_RECT_F {
        left: 12.0,
        top: 0.0,
        right: size.width - 72.0,
        bottom: 32.0,
    };
    rt.DrawText(
        &preedit_w,
        &inner.text_format_header,
        &preedit_rect,
        &brush_accent_text,
        windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
        windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
    );

    // Page indicator on right of header
    let page_str = format!("{}/{}", page.page_index + 1, page.total_pages);
    let page_w: Vec<u16> = page_str.encode_utf16().collect();
    let page_rect = D2D_RECT_F {
        left: size.width - 72.0,
        top: 0.0,
        right: size.width - 12.0,
        bottom: 32.0,
    };
    rt.DrawText(
        &page_w,
        &inner.text_format_badge,
        &page_rect,
        &brush_muted,
        windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
        windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
    );

    // Candidate rows
    let row_h = 30.0_f32;
    for (idx, item) in page.items.iter().enumerate() {
        let top = 34.0 + (idx as f32) * row_h;
        let bottom = top + row_h;
        let is_selected = idx == page.selected_in_page;

        if is_selected {
            let pill = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: 6.0,
                    top: top + 2.0,
                    right: size.width - 6.0,
                    bottom: bottom - 2.0,
                },
                radiusX: 5.0,
                radiusY: 5.0,
            };
            rt.FillRoundedRectangle(&pill, &brush_accent_bg);
        }

        let label = if idx == 9 {
            "0.".to_string()
        } else {
            format!("{}.", idx + 1)
        };
        let label_w: Vec<u16> = label.encode_utf16().collect();
        let label_rect = D2D_RECT_F {
            left: 14.0,
            top,
            right: 36.0,
            bottom,
        };
        rt.DrawText(
            &label_w,
            &inner.text_format_header,
            &label_rect,
            if is_selected {
                &brush_accent_text
            } else {
                &brush_muted
            },
            windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
            windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
        );

        let mut cand_left = 38.0_f32;
        if !item.suffix.is_empty() {
            let suffix_w: Vec<u16> = item.suffix.encode_utf16().collect();
            let suffix_width = (item.suffix.len() as f32 * 8.5 + 8.0).min(70.0);
            let suffix_rect = D2D_RECT_F {
                left: cand_left,
                top,
                right: cand_left + suffix_width,
                bottom,
            };
            rt.DrawText(
                &suffix_w,
                &inner.text_format_header,
                &suffix_rect,
                if is_selected {
                    &brush_accent_text
                } else {
                    &brush_muted
                },
                windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
            );
            cand_left += suffix_width;
        }

        let cand_w: Vec<u16> = item.text.encode_utf16().collect();
        let cand_rect = D2D_RECT_F {
            left: cand_left,
            top,
            right: size.width - 12.0,
            bottom,
        };
        rt.DrawText(
            &cand_w,
            &inner.text_format_cand,
            &cand_rect,
            &brush_text,
            windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
            windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
        );
    }

    // Subtle outer border
    let border_rect = D2D_RECT_F {
        left: 0.5,
        top: 0.5,
        right: size.width - 0.5,
        bottom: size.height - 0.5,
    };
    rt.DrawRectangle(&border_rect, &brush_border, 1.0, None);

    if rt.EndDraw(None, None) == Err(D2DERR_RECREATE_TARGET.into()) {
        inner.render_target = None;
    }
}
