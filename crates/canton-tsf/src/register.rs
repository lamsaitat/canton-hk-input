use crate::guids::{
    guid_to_braced_string, CLSID_CANTON_TEXT_SERVICE, GUID_CANTON_DISPLAY_ATTRIBUTE,
    GUID_CANTON_PROFILE, LANGID_ZH_TW,
};
use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use windows::core::{w, GUID, HRESULT, PCWSTR};
use windows::Win32::Foundation::{HMODULE, MAX_PATH};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, HKEY_CLASSES_ROOT,
    KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ,
};
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, ITfCategoryMgr,
    ITfInputProcessorProfileMgr, GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER, GUID_TFCAT_TIPCAP_COMLESS,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT, GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
    GUID_TFCAT_TIPCAP_SECUREMODE, GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED, GUID_TFCAT_TIP_KEYBOARD,
};

const PROFILE_NAME: &str = "CantonHK (廣東拼音)";

const CATEGORIES: &[GUID] = &[
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_SECUREMODE,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
    GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
    GUID_TFCAT_TIPCAP_COMLESS,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
];

fn to_wide(s: &str) -> Vec<u16> {
    OsString::from(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

unsafe fn get_dll_path(hmodule: HMODULE) -> windows::core::Result<Vec<u16>> {
    let mut buf = vec![0u16; MAX_PATH as usize];
    let len = GetModuleFileNameW(hmodule, &mut buf);
    if len == 0 {
        return Err(windows::core::Error::from_win32());
    }
    buf.truncate(len as usize + 1);
    Ok(buf)
}

unsafe fn set_reg_sz(subkey: &str, value_name: Option<&str>, data_w: &[u16]) -> windows::core::Result<()> {
    let subkey_w = to_wide(subkey);
    let mut hkey = Default::default();
    let status = RegCreateKeyExW(
        HKEY_CLASSES_ROOT,
        PCWSTR(subkey_w.as_ptr()),
        0,
        None,
        REG_OPTION_NON_VOLATILE,
        KEY_WRITE,
        None,
        &mut hkey,
        None,
    );
    if status.is_err() {
        return Err(windows::core::Error::from(status));
    }
    let val_w = value_name.map(to_wide);
    let val_ptr = val_w
        .as_ref()
        .map_or(PCWSTR::null(), |v| PCWSTR(v.as_ptr()));
    let bytes = std::slice::from_raw_parts(
        data_w.as_ptr() as *const u8,
        data_w.len() * std::mem::size_of::<u16>(),
    );
    let set_res = RegSetValueExW(hkey, val_ptr, 0, REG_SZ, Some(bytes));
    let _ = RegCloseKey(hkey);
    if set_res.is_err() {
        return Err(windows::core::Error::from(set_res));
    }
    Ok(())
}

pub unsafe fn register_server(hmodule: HMODULE) -> HRESULT {
    let co_init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    let res = register_server_inner(hmodule);
    if co_init.is_ok() {
        CoUninitialize();
    }
    match res {
        Ok(()) => HRESULT(0),
        Err(e) => e.code(),
    }
}

unsafe fn register_server_inner(hmodule: HMODULE) -> windows::core::Result<()> {
    let clsid_str = guid_to_braced_string(&CLSID_CANTON_TEXT_SERVICE);
    let clsid_key = format!("CLSID\\{}", clsid_str);
    let inproc_key = format!("{}\\InprocServer32", clsid_key);

    let name_w = to_wide(PROFILE_NAME);
    set_reg_sz(&clsid_key, None, &name_w)?;

    let dll_path_w = get_dll_path(hmodule)?;
    set_reg_sz(&inproc_key, None, &dll_path_w)?;
    set_reg_sz(&inproc_key, Some("ThreadingModel"), &to_wide("Apartment"))?;

    let profile_mgr: ITfInputProcessorProfileMgr =
        CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
    let desc_w: Vec<u16> = PROFILE_NAME.encode_utf16().collect();
    let icon_path_w = &dll_path_w[..dll_path_w.len() - 1];
    profile_mgr.RegisterProfile(
        &CLSID_CANTON_TEXT_SERVICE,
        LANGID_ZH_TW,
        &GUID_CANTON_PROFILE,
        &desc_w,
        icon_path_w,
        0,
        None,
        0,
        true,
        0,
    )?;

    let cat_mgr: ITfCategoryMgr =
        CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
    for cat in CATEGORIES {
        cat_mgr.RegisterCategory(&CLSID_CANTON_TEXT_SERVICE, cat, &CLSID_CANTON_TEXT_SERVICE)?;
    }
    cat_mgr.RegisterCategory(
        &CLSID_CANTON_TEXT_SERVICE,
        &GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
        &GUID_CANTON_DISPLAY_ATTRIBUTE,
    )?;

    Ok(())
}

pub unsafe fn unregister_server() -> HRESULT {
    let co_init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    let res = unregister_server_inner();
    if co_init.is_ok() {
        CoUninitialize();
    }
    match res {
        Ok(()) => HRESULT(0),
        Err(e) => e.code(),
    }
}

unsafe fn unregister_server_inner() -> windows::core::Result<()> {
    if let Ok(cat_mgr) =
        CoCreateInstance::<_, ITfCategoryMgr>(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)
    {
        for cat in CATEGORIES {
            let _ = cat_mgr.UnregisterCategory(
                &CLSID_CANTON_TEXT_SERVICE,
                cat,
                &CLSID_CANTON_TEXT_SERVICE,
            );
        }
        let _ = cat_mgr.UnregisterCategory(
            &CLSID_CANTON_TEXT_SERVICE,
            &GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
            &GUID_CANTON_DISPLAY_ATTRIBUTE,
        );
    }

    if let Ok(profile_mgr) = CoCreateInstance::<_, ITfInputProcessorProfileMgr>(
        &CLSID_TF_InputProcessorProfiles,
        None,
        CLSCTX_INPROC_SERVER,
    ) {
        let _ = profile_mgr.UnregisterProfile(
            &CLSID_CANTON_TEXT_SERVICE,
            LANGID_ZH_TW,
            &GUID_CANTON_PROFILE,
            0,
        );
    }

    let clsid_str = guid_to_braced_string(&CLSID_CANTON_TEXT_SERVICE);
    let clsid_key = to_wide(&format!("CLSID\\{}", clsid_str));
    let _ = RegDeleteTreeW(HKEY_CLASSES_ROOT, PCWSTR(clsid_key.as_ptr()));
    let _ = w!("unused");
    Ok(())
}
