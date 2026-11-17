use crate::guids::GUID_CANTON_DISPLAY_ATTRIBUTE;
use std::sync::atomic::{AtomicUsize, Ordering};
use windows::core::{implement, w, BSTR, GUID, PWSTR};
use windows::Win32::Foundation::{BOOL, E_INVALIDARG, S_FALSE};
use windows::Win32::UI::TextServices::{
    IEnumTfDisplayAttributeInfo, IEnumTfDisplayAttributeInfo_Impl, ITfDisplayAttributeInfo,
    ITfDisplayAttributeInfo_Impl, TF_ATTR_INPUT, TF_CT_NONE, TF_DISPLAYATTRIBUTE,
    TF_LS_DOT,
};

#[implement(ITfDisplayAttributeInfo)]
pub struct CantonDisplayAttributeInfo {
    attr: TF_DISPLAYATTRIBUTE,
}

impl CantonDisplayAttributeInfo {
    pub fn new() -> Self {
        Self {
            attr: TF_DISPLAYATTRIBUTE {
                crText: Default::default(),
                crBk: Default::default(),
                lsStyle: TF_LS_DOT,
                fBoldLine: BOOL(0),
                crLine: windows::Win32::UI::TextServices::TF_DA_COLOR {
                    r#type: TF_CT_NONE,
                    Anonymous: Default::default(),
                },
                bAttr: TF_ATTR_INPUT,
            },
        }
    }
}

impl ITfDisplayAttributeInfo_Impl for CantonDisplayAttributeInfo_Impl {
    fn GetGUID(&self) -> windows::core::Result<GUID> {
        Ok(GUID_CANTON_DISPLAY_ATTRIBUTE)
    }

    fn GetDescription(&self) -> windows::core::Result<BSTR> {
        Ok(BSTR::from("CantonHK Composition"))
    }

    fn GetAttributeInfo(&self, pda: *mut TF_DISPLAYATTRIBUTE) -> windows::core::Result<()> {
        if pda.is_null() {
            return Err(E_INVALIDARG.into());
        }
        unsafe {
            *pda = self.attr;
        }
        Ok(())
    }

    fn SetAttributeInfo(&self, _pda: *const TF_DISPLAYATTRIBUTE) -> windows::core::Result<()> {
        Ok(())
    }

    fn Reset(&self) -> windows::core::Result<()> {
        Ok(())
    }
}

#[implement(IEnumTfDisplayAttributeInfo)]
pub struct CantonEnumDisplayAttributeInfo {
    index: AtomicUsize,
}

impl CantonEnumDisplayAttributeInfo {
    pub fn new() -> Self {
        Self {
            index: AtomicUsize::new(0),
        }
    }
}

impl IEnumTfDisplayAttributeInfo_Impl for CantonEnumDisplayAttributeInfo_Impl {
    fn Clone(&self) -> windows::core::Result<IEnumTfDisplayAttributeInfo> {
        let clone = CantonEnumDisplayAttributeInfo {
            index: AtomicUsize::new(self.index.load(Ordering::Relaxed)),
        };
        Ok(clone.into())
    }

    fn Next(
        &self,
        ulcount: u32,
        rginfo: *mut Option<ITfDisplayAttributeInfo>,
        pcfetched: *mut u32,
    ) -> windows::core::Result<()> {
        if rginfo.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let mut fetched = 0u32;
        if ulcount > 0 && self.index.load(Ordering::Relaxed) == 0 {
            self.index.store(1, Ordering::Relaxed);
            let info: ITfDisplayAttributeInfo = CantonDisplayAttributeInfo::new().into();
            unsafe {
                *rginfo = Some(info);
            }
            fetched = 1;
        }
        if !pcfetched.is_null() {
            unsafe {
                *pcfetched = fetched;
            }
        }
        if fetched == ulcount {
            Ok(())
        } else {
            Err(S_FALSE.into())
        }
    }

    fn Reset(&self) -> windows::core::Result<()> {
        self.index.store(0, Ordering::Relaxed);
        Ok(())
    }

    fn Skip(&self, ulcount: u32) -> windows::core::Result<()> {
        if ulcount > 0 {
            self.index.store(1, Ordering::Relaxed);
        }
        Ok(())
    }
}

#[allow(dead_code)]
fn _unused_pwstr(_p: PWSTR) {
    let _ = w!("");
}
