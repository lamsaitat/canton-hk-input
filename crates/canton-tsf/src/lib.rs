#![allow(non_snake_case)]

mod display_attr;
mod guids;
mod langbar;
mod register;
mod text_service;

use crate::guids::CLSID_CANTON_TEXT_SERVICE;
use crate::text_service::CantonTextService;
use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use windows::core::{implement, GUID, HRESULT, IUnknown, Interface};
use windows::Win32::Foundation::{
    BOOL, CLASS_E_CLASSNOTAVAILABLE, E_INVALIDARG, HMODULE, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;

static DLL_HMODULE: AtomicUsize = AtomicUsize::new(0);
static OBJECT_COUNT: AtomicUsize = AtomicUsize::new(0);

#[implement(IClassFactory)]
struct CantonClassFactory;

impl CantonClassFactory {
    fn new() -> Self {
        OBJECT_COUNT.fetch_add(1, Ordering::Relaxed);
        Self
    }
}

impl Drop for CantonClassFactory {
    fn drop(&mut self) {
        OBJECT_COUNT.fetch_sub(1, Ordering::Relaxed);
    }
}

impl IClassFactory_Impl for CantonClassFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Option<&IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut c_void,
    ) -> windows::core::Result<()> {
        if ppvobject.is_null() || riid.is_null() {
            return Err(E_INVALIDARG.into());
        }
        unsafe {
            *ppvobject = std::ptr::null_mut();
        }
        if punkouter.is_some() {
            return Err(windows::Win32::Foundation::CLASS_E_NOAGGREGATION.into());
        }
        let service: IUnknown = CantonTextService::new().into();
        unsafe { service.query(riid, ppvobject).ok() }
    }

    fn LockServer(&self, flock: BOOL) -> windows::core::Result<()> {
        if flock.as_bool() {
            OBJECT_COUNT.fetch_add(1, Ordering::Relaxed);
        } else {
            OBJECT_COUNT.fetch_sub(1, Ordering::Relaxed);
        }
        Ok(())
    }
}

#[no_mangle]
pub extern "system" fn DllMain(
    hinst_dll: HMODULE,
    fdw_reason: u32,
    _lpv_reserved: *const c_void,
) -> BOOL {
    if fdw_reason == DLL_PROCESS_ATTACH {
        DLL_HMODULE.store(hinst_dll.0 as usize, Ordering::Relaxed);
    }
    BOOL(1)
}

#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if rclsid.is_null() || riid.is_null() || ppv.is_null() {
        return E_INVALIDARG;
    }
    *ppv = std::ptr::null_mut();
    if *rclsid != CLSID_CANTON_TEXT_SERVICE {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory: IUnknown = CantonClassFactory::new().into();
    factory.query(riid, ppv)
}

#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if OBJECT_COUNT.load(Ordering::Relaxed) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

#[no_mangle]
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    let raw = DLL_HMODULE.load(Ordering::Relaxed);
    let hmodule = HMODULE(raw as *mut c_void);
    register::register_server(hmodule)
}

#[no_mangle]
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    register::unregister_server()
}
