use crate::guids::{
    guid_to_braced_string, CLSID_CANTON_TEXT_SERVICE, GUID_CANTON_DISPLAY_ATTRIBUTE,
    GUID_CANTON_PROFILE, LANGID_ZH_TW,
};
use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use windows::core::{w, GUID, HRESULT, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HMODULE, MAX_PATH, RECT};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject, DrawTextW,
    SelectObject, SetBkMode, SetTextColor, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_QUALITY, DIB_RGB_COLORS, DT_CENTER,
    DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, FW_BOLD, OUT_DEFAULT_PRECIS, TRANSPARENT,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW, HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE,
    REG_OPTION_NON_VOLATILE, REG_SZ,
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

    // Generate `cantonhk.ico` ("港") next to the DLL for the Win+Space & Taskbar profile icon
    let dll_str_len = dll_path_w.iter().position(|&c| c == 0).unwrap_or(dll_path_w.len());
    let dll_path = PathBuf::from(OsString::from_wide(&dll_path_w[..dll_str_len]));
    let ico_path = dll_path.with_file_name("cantonhk.ico");
    let icon_file_w: Vec<u16> = if write_profile_icon(&ico_path).is_ok() {
        ico_path.to_string_lossy().encode_utf16().collect()
    } else {
        dll_path_w[..dll_str_len].to_vec()
    };

    let profile_mgr: ITfInputProcessorProfileMgr =
        CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
    let desc_w: Vec<u16> = PROFILE_NAME.encode_utf16().collect();
    profile_mgr.RegisterProfile(
        &CLSID_CANTON_TEXT_SERVICE,
        LANGID_ZH_TW,
        &GUID_CANTON_PROFILE,
        &desc_w,
        &icon_file_w,
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

unsafe fn is_taskbar_light() -> bool {
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
    false
}

unsafe fn write_profile_icon(path: &PathBuf) -> windows::core::Result<()> {
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
    let hbm = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0)?;
    let hdc = CreateCompatibleDC(None);
    let old_bm = SelectObject(hdc, hbm);

    let text_color = if is_taskbar_light() {
        COLORREF(0x001A1A1A)
    } else {
        COLORREF(0x00FFFFFF)
    };

    let hfont = CreateFontW(
        -24,
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

    let mut text_w: Vec<u16> = "港".encode_utf16().collect();
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

    // Build standard 32-bit ARGB bottom-up ICO payload
    let xor_bytes = (SIZE * SIZE * 4) as u32;
    let and_bytes = (SIZE * 4) as u32;
    let total_img_bytes = 40 + xor_bytes + and_bytes;

    let mut ico = Vec::with_capacity(22 + total_img_bytes as usize);
    // ICONDIR
    ico.extend_from_slice(&0u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    // ICONDIRENTRY
    ico.push(SIZE as u8);
    ico.push(SIZE as u8);
    ico.push(0);
    ico.push(0);
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&32u16.to_le_bytes());
    ico.extend_from_slice(&total_img_bytes.to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());
    // BITMAPINFOHEADER (biHeight = 2 * SIZE in ICO format)
    ico.extend_from_slice(&40u32.to_le_bytes());
    ico.extend_from_slice(&SIZE.to_le_bytes());
    ico.extend_from_slice(&(SIZE * 2).to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&32u16.to_le_bytes());
    ico.extend_from_slice(&0u32.to_le_bytes());
    ico.extend_from_slice(&xor_bytes.to_le_bytes());
    ico.extend_from_slice(&0i32.to_le_bytes());
    ico.extend_from_slice(&0i32.to_le_bytes());
    ico.extend_from_slice(&0u32.to_le_bytes());
    ico.extend_from_slice(&0u32.to_le_bytes());

    if !bits_ptr.is_null() {
        let pixels = std::slice::from_raw_parts(bits_ptr as *const u32, (SIZE * SIZE) as usize);
        // Flip vertically (DIB was top-down, ICO expects bottom-up) and set alpha on non-zero pixels
        for y in (0..SIZE as usize).rev() {
            for x in 0..SIZE as usize {
                let mut px = pixels[y * (SIZE as usize) + x];
                if (px & 0x00FF_FFFF) != 0 {
                    px |= 0xFF00_0000;
                }
                ico.extend_from_slice(&px.to_le_bytes());
            }
        }
    } else {
        ico.resize(ico.len() + xor_bytes as usize, 0);
    }
    ico.resize(ico.len() + and_bytes as usize, 0);

    let _ = DeleteObject(hbm);
    let _ = std::fs::write(path, &ico);
    Ok(())
}
