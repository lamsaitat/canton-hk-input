use crate::guids::GUID_CANTON_LANGBAR_ITEM;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows::core::{implement, BSTR, GUID, HRESULT, IUnknown, Interface};
use windows::Win32::Foundation::{BOOL, E_FAIL, E_INVALIDARG, POINT, RECT, S_OK};
use windows::Win32::UI::TextServices::{
    ITfLangBarItem, ITfLangBarItemButton, ITfLangBarItemButton_Impl, ITfLangBarItemSink,
    ITfLangBarItem_Impl, ITfMenu, ITfSource, ITfSource_Impl, TfLBIClick, TF_LANGBARITEMINFO,
    TF_LBI_BMPF_VERTICAL, TF_LBI_STATUS_BTN_TOGGLED, TF_LBI_STYLE_BTN_BUTTON,
    TF_LBI_STYLE_SHOWNINTRAY, TF_LBI_TEXT,
};
use windows::Win32::UI::WindowsAndMessaging::HICON;

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
                let _ = sink.OnUpdate(TF_LBI_TEXT | TF_LBI_BMPF_VERTICAL);
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
            info.clsidService = crate::guids::CLSID_CANTON_TEXT_SERVICE;
            info.guidItem = GUID_CANTON_LANGBAR_ITEM;
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
            "CantonHK: 中文模式 (Click or press Shift for English)"
        } else {
            "CantonHK: English Mode (Click or press Shift for Chinese)"
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
        Err(E_FAIL.into())
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

#[allow(dead_code)]
fn _check_s_ok() -> HRESULT {
    S_OK
}
