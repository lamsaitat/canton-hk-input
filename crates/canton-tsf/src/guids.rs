use windows::core::GUID;

/// COM Class ID for CantonHK Text Input Processor.
/// {8E4C7D2A-9B31-4F6E-A1C5-3D7E8F901234}
pub const CLSID_CANTON_TEXT_SERVICE: GUID =
    GUID::from_u128(0x8E4C7D2A_9B31_4F6E_A1C5_3D7E8F901234);

/// TSF Input Profile GUID for CantonHK (zh-TW).
/// {8E4C7D2A-9B31-4F6E-A1C5-3D7E8F901235}
pub const GUID_CANTON_PROFILE: GUID =
    GUID::from_u128(0x8E4C7D2A_9B31_4F6E_A1C5_3D7E8F901235);

/// Display Attribute GUID for active composition underline.
/// {8E4C7D2A-9B31-4F6E-A1C5-3D7E8F901236}
pub const GUID_CANTON_DISPLAY_ATTRIBUTE: GUID =
    GUID::from_u128(0x8E4C7D2A_9B31_4F6E_A1C5_3D7E8F901236);

/// Standard Windows TSF Language Bar Input Mode item GUID (`GUID_LBI_INPUTMODE`).
/// Windows 10/11 Taskbar requires this exact GUID to show the IME mode icon in the system tray.
/// {2C77A81E-41CC-4178-A3A7-5F8A987568E6}
pub const GUID_LBI_INPUTMODE: GUID =
    GUID::from_u128(0x2C77A81E_41CC_4178_A3A7_5F8A987568E6);

/// Traditional Chinese (Taiwan) LANGID: 0x0404
pub const LANGID_ZH_TW: u16 = 0x0404;

pub fn guid_to_braced_string(guid: &GUID) -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        guid.data1,
        guid.data2,
        guid.data3,
        guid.data4[0],
        guid.data4[1],
        guid.data4[2],
        guid.data4[3],
        guid.data4[4],
        guid.data4[5],
        guid.data4[6],
        guid.data4[7],
    )
}
