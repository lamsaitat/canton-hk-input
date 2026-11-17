use crate::guids::{CLSID_CANTON_TEXT_SERVICE, GUID_LBI_INPUTMODE};
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows::core::{implement, w, BSTR, GUID, HRESULT, IUnknown, Interface};
use windows::Win32::Foundation::{BOOL, COLORREF, E_INVALIDARG, POINT, RECT, S_OK};
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject,
    DrawTextW, SelectObject, SetBkMode, SetTextColor, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_QUALITY, DIB_RGB_COLORS, DT_CENTER, DT_SINGLELINE,
    DT_VCENTER, FF_DONTCARE, FW_BOLD, OUT_DEFAULT_PRECIS, TRANSPARENT,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ,
};
use windows::Win32::UI::TextServices::{
    ITfLangBarItem, ITfLangBarItemButton, ITfLangBarItemButton_Impl, ITfLangBarItemSink,
    ITfLangBarItem_Impl, ITfMenu, ITfSource, ITfSource_Impl, TfLBIClick, TF_LANGBARITEMINFO,
    TF_LBI_ICON, TF_LBI_STATUS, TF_LBI_STATUS_BTN_TOGGLED, TF_LBI_STYLE_BTN_BUTTON,
    TF_LBI_STYLE_SHOWNINTRAY, TF_LBI_TEXT,
};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO};

#[implement(ITfLangBarItemButton, ITfLangBarItem, ITfSource)]
pub struct CantonLangBarButton {
    chinese_mode: Arc<AtomicBool>,
    on_toggle: Box<dyn Fn()>,
    sink: RefCell<Option<ITfLangBarItemSink>>,
    sink_cookie: RefCell<u32>,
}

impl CantonLangBarButton {
    pub fn new(chinese_mode: Arc<AtomicBool>, on_toggle: Box<dyn Fn()>) -> Self {
        Self {
            chinese_mode,
            on_toggle,
            sink: RefCell::new(None),
            sink_cookie: RefCell::new(0),
        }
    }

    pub fn notify_update(&self) {
        if let Some(sink) = self.sink.borrow().as_ref() {
            unsafe {
                let _ = sink.OnUpdate(TF_LBI_ICON | TF_LBI_TEXT | TF_LBI_STATUS);
            }
        }
    }
}

impl ITfLangBarItem_Impl for CantonLangBarButton_Impl {
    fn GetInfo(&self, pinfo: *mut TF_LANGBARITEMINFO) -> windows::core::Result<()> {
        if pinfo.is_null() {
            return Err(E_INVALIDARG.into());
        }
        unsafe {
            let info = &mut *pinfo;
            info.clsidService = CLSID_CANTON_TEXT_SERVICE;
            info.guidItem = GUID_LBI_INPUTMODE;
            info.dwStyle = TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY;
            info.ulSort = 0;
            let desc: Vec<u16> = "CantonHK Mode (中/英)".encode_utf16().collect();
            let len = desc.len().min(info.szDescription.len() - 1);
            info.szDescription[..len].copy_from_slice(&desc[..len]);
            info.szDescription[len] = 0;
        }
        Ok(())
    }

    fn GetStatus(&self) -> windows::core::Result<u32> {
        if self.chinese_mode.load(Ordering::Relaxed) {
            Ok(TF_LBI_STATUS_BTN_TOGGLED)
        } else {
            Ok(0)
        }
    }

    fn Show(&self, _fshow: BOOL) -> windows::core::Result<()> {
        Ok(())
    }

    fn GetTooltipString(&self) -> windows::core::Result<BSTR> {
        let tip = if self.chinese_mode.load(Ordering::Relaxed) {
            "CantonHK: 廣東拼音 (Click or press Shift for English)"
        } else {
            "CantonHK: English Mode (Click or press Shift for 廣東拼音)"
        };
        Ok(BSTR::from(tip))
    }
}

impl ITfLangBarItemButton_Impl for CantonLangBarButton_Impl {
    fn OnClick(
        &self,
        _click: TfLBIClick,
        _pt: &POINT,
        _prcarea: *const RECT,
    ) -> windows::core::Result<()> {
        (self.on_toggle)();
        Ok(())
    }

    fn InitMenu(&self, _pmenu: Option<&ITfMenu>) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnMenuSelect(&self, _wid: u32) -> windows::core::Result<()> {
        Ok(())
    }

    fn GetIcon(&self) -> windows::core::Result<HICON> {
        let chinese = self.chinese_mode.load(Ordering::Relaxed);
        let text = if chinese { "中" } else { "英" };
        unsafe { render_glyph_icon(text, -22) }
    }

    fn GetText(&self) -> windows::core::Result<BSTR> {
        if self.chinese_mode.load(Ordering::Relaxed) {
            Ok(BSTR::from("中"))
        } else {
            Ok(BSTR::from("英"))
        }
    }
}

impl ITfSource_Impl for CantonLangBarButton_Impl {
    fn AdviseSink(
        &self,
        riid: *const GUID,
        punk: Option<&IUnknown>,
    ) -> windows::core::Result<u32> {
        if riid.is_null() {
            return Err(E_INVALIDARG.into());
        }
        unsafe {
            if *riid != ITfLangBarItemSink::IID {
                return Err(HRESULT(-2147220990).into()); // CONNECT_E_CANNOTCONNECT
            }
            let Some(punk) = punk else {
                return Err(E_INVALIDARG.into());
            };
            let sink: ITfLangBarItemSink = punk.cast()?;
            *self.sink.borrow_mut() = Some(sink);
            let mut cookie = self.sink_cookie.borrow_mut();
            *cookie += 1;
            Ok(*cookie)
        }
    }

    fn UnadviseSink(&self, dwcookie: u32) -> windows::core::Result<()> {
        if *self.sink_cookie.borrow() == dwcookie {
            *self.sink.borrow_mut() = None;
        }
        Ok(())
    }
}

fn is_system_taskbar_light() -> bool {
    unsafe {
        let mut hkey = Default::default();
        let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey, 0, KEY_READ, &mut hkey).is_ok() {
            let mut val: u32 = 0;
            let mut size = std::mem::size_of::<u32>() as u32;
            let ok = RegQueryValueExW(
                hkey,
                w!("SystemUsesLightTheme"),
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
    false
}

unsafe fn render_glyph_icon(text: &str, font_height: i32) -> windows::core::Result<HICON> {
    const SIZE: i32 = 32;

    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: SIZE,
            biHeight: -SIZE,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
    let hbm_color = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0)?;
    let hdc = CreateCompatibleDC(None);
    let old_bm = SelectObject(hdc, hbm_color);

    let light_taskbar = is_system_taskbar_light();
    let text_color = if light_taskbar {
        COLORREF(0x001A1A1A)
    } else {
        COLORREF(0x00FFFFFF)
    };

    // Draw text onto a clean DIB; GDI leaves alpha=0 on drawn pixels so we fix up alpha below
    let hfont = CreateFontW(
        font_height,
        0,
        0,
        0,
        FW_BOLD.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0 as u32,
        CLIP_DEFAULT_PRECIS.0 as u32,
        DEFAULT_QUALITY.0 as u32,
        FF_DONTCARE.0 as u32,
        w!("Microsoft JhengHei UI"),
    );
    let old_font = SelectObject(hdc, hfont);
    SetBkMode(hdc, TRANSPARENT);
    SetTextColor(hdc, text_color);

    let mut text_w: Vec<u16> = text.encode_utf16().collect();
    let mut rc = RECT {
        left: 0,
        top: 0,
        right: SIZE,
        bottom: SIZE,
    };
    DrawTextW(
        hdc,
        &mut text_w,
        &mut rc,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );

    SelectObject(hdc, old_font);
    let _ = DeleteObject(hfont);
    SelectObject(hdc, old_bm);
    let _ = DeleteDC(hdc);

    // Fix alpha channel: any pixel touched by GDI (non-zero RGB) gets alpha=0xFF
    if !bits_ptr.is_null() {
        let pixels = std::slice::from_raw_parts_mut(bits_ptr as *mut u32, (SIZE * SIZE) as usize);
        for px in pixels.iter_mut() {
            let b = *px & 0xFF;
            let g = (*px >> 8) & 0xFF;
            let r = (*px >> 16) & 0xFF;
            let luma = if light_taskbar {
                // If dark text was drawn on 0x000000, invert-detect isn't needed if we draw white first;
                // Wait: on light taskbar, dark text 0x1A1A1A is still non-zero!
                (r | g | b) != 0
            } else {
                (r | g | b) != 0
            };
            if luma {
                *px |= 0xFF00_0000;
            }
        }
    }

    let hbm_mask = CreateBitmap(SIZE, SIZE, 1, 1, None);
    let icon_info = ICONINFO {
        fIcon: BOOL(1),
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: hbm_mask,
        hbmColor: hbm_color,
    };
    let hicon = CreateIconIndirect(&icon_info);

    let _ = DeleteObject(hbm_mask);
    let _ = DeleteObject(hbm_color);

    hicon
}

#[allow(dead_code)]
fn _check_s_ok() -> HRESULT {
    S_OK
}
