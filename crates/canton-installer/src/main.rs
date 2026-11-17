#![windows_subsystem = "windows"]

use std::ffi::OsString;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_DELAY_UNTIL_REBOOT};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, HKEY_LOCAL_MACHINE, KEY_WRITE,
    REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ,
};
use windows::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDCANCEL, IDNO, IDYES, MB_ICONERROR, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK,
    MB_YESNO, MB_YESNOCANCEL, SW_SHOWNORMAL,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const EMBEDDED_DLL_X64: &[u8] = include_bytes!("../../../dist/x64/canton_ime.dll");

const INSTALL_DIR: &str = r"C:\Program Files\CantonHK";
const UNINSTALL_REG_KEY: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\CantonHK";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Install,
    Uninstall,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let silent = args.iter().any(|a| a.eq_ignore_ascii_case("--silent") || a == "/S");

    let explicit_action = if args
        .iter()
        .any(|a| a.eq_ignore_ascii_case("--uninstall") || a == "/U")
    {
        Some(Action::Uninstall)
    } else if args
        .iter()
        .any(|a| a.eq_ignore_ascii_case("--install") || a == "/I")
    {
        Some(Action::Install)
    } else {
        None
    };

    let dll_path = PathBuf::from(INSTALL_DIR).join("x64").join("canton_ime.dll");
    let already_installed = dll_path.exists();

    let action = match explicit_action {
        Some(a) => a,
        None => {
            if already_installed {
                let choice = msg_box(
                    "CantonHK (廣東拼音) Setup",
                    "CantonHK (廣東拼音) is already installed.\n\n\
                     • Yes = Reinstall / Update\n\
                     • No = Uninstall CantonHK\n\
                     • Cancel = Exit",
                    MB_YESNOCANCEL.0 | MB_ICONQUESTION.0,
                );
                if choice == IDYES.0 {
                    Action::Install
                } else if choice == IDNO.0 {
                    Action::Uninstall
                } else {
                    return;
                }
            } else {
                let choice = msg_box(
                    "CantonHK (廣東拼音) Setup",
                    "Install CantonHK (廣東拼音) Input Method for Windows 10/11?\n\n\
                     This will install CantonHK into Program Files and register it under Traditional Chinese (zh-TW).",
                    MB_YESNO.0 | MB_ICONQUESTION.0,
                );
                if choice == IDYES.0 {
                    Action::Install
                } else {
                    return;
                }
            }
        }
    };

    if !is_admin() {
        relaunch_as_admin(action, silent);
        return;
    }

    let result = match action {
        Action::Install => perform_install(silent),
        Action::Uninstall => perform_uninstall(silent),
    };

    if let Err(err) = result {
        if !silent {
            msg_box(
                "CantonHK Setup Error",
                &format!("Operation failed:\n\n{}", err),
                MB_OK.0 | MB_ICONERROR.0,
            );
        }
        std::process::exit(1);
    }
}

fn perform_install(silent: bool) -> Result<(), String> {
    let install_root = PathBuf::from(INSTALL_DIR);
    let x64_dir = install_root.join("x64");
    let dll_path = x64_dir.join("canton_ime.dll");
    let uninstaller_path = install_root.join("Uninstall.exe");

    // Unregister any existing installation first
    if dll_path.exists() {
        let _ = run_regsvr32(&dll_path, true);
        remove_or_rename_locked(&dll_path);
    }

    fs::create_dir_all(&x64_dir).map_err(|e| format!("Failed to create install folder: {e}"))?;
    fs::write(&dll_path, EMBEDDED_DLL_X64)
        .map_err(|e| format!("Failed to write canton_ime.dll: {e}"))?;

    if let Ok(self_exe) = std::env::current_exe() {
        if self_exe != uninstaller_path {
            let _ = fs::copy(&self_exe, &uninstaller_path);
        }
    }

    // Grant AppContainer / UWP / Windows 11 Shell Read & Execute permissions
    grant_appcontainer_acl(&install_root)?;

    // Register 64-bit TSF COM server
    run_regsvr32(&dll_path, false)?;

    // Register Add/Remove Programs entry
    let _ = write_uninstall_registry(&install_root, &uninstaller_path);

    if !silent {
        msg_box(
            "CantonHK (廣東拼音) Installed",
            "CantonHK (廣東拼音) has been installed successfully!\n\n\
             Press Win + Space to switch to 'CantonHK (廣東拼音)'.\n\n\
             (If it does not appear in Win + Space, add 'Chinese (Traditional, Taiwan)' in Windows Settings -> Time & language -> Language & region.)",
            MB_OK.0 | MB_ICONINFORMATION.0,
        );
    }
    Ok(())
}

fn perform_uninstall(silent: bool) -> Result<(), String> {
    let install_root = PathBuf::from(INSTALL_DIR);
    let dll_path = install_root.join("x64").join("canton_ime.dll");
    let uninstaller_path = install_root.join("Uninstall.exe");

    if dll_path.exists() {
        let _ = run_regsvr32(&dll_path, true);
        remove_or_rename_locked(&dll_path);
    }

    delete_uninstall_registry();

    if uninstaller_path.exists() {
        let running_as_uninstaller = std::env::current_exe()
            .map(|p| p.eq_ignore_ascii_case_or_exact(&uninstaller_path))
            .unwrap_or(false);
        if running_as_uninstaller {
            schedule_delete_on_reboot(&uninstaller_path);
        } else {
            let _ = fs::remove_file(&uninstaller_path);
        }
    }

    // Remove empty directories if possible
    let _ = fs::remove_dir(install_root.join("x64"));
    let _ = fs::remove_dir(&install_root);

    if !silent {
        msg_box(
            "CantonHK (廣東拼音) Uninstalled",
            "CantonHK (廣東拼音) has been unregistered and removed.",
            MB_OK.0 | MB_ICONINFORMATION.0,
        );
    }
    Ok(())
}

trait PathEqExt {
    fn eq_ignore_ascii_case_or_exact(&self, other: &Path) -> bool;
}

impl PathEqExt for PathBuf {
    fn eq_ignore_ascii_case_or_exact(&self, other: &Path) -> bool {
        self.to_string_lossy()
            .eq_ignore_ascii_case(&other.to_string_lossy())
    }
}

fn remove_or_rename_locked(path: &Path) {
    if fs::remove_file(path).is_ok() {
        return;
    }
    let old_name = format!(
        "{}.old.{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    );
    let old_path = path.with_file_name(old_name);
    if fs::rename(path, &old_path).is_ok() {
        schedule_delete_on_reboot(&old_path);
    } else {
        schedule_delete_on_reboot(path);
    }
}

fn schedule_delete_on_reboot(path: &Path) {
    let w = to_wide(&path.to_string_lossy());
    unsafe {
        let _ = MoveFileExW(PCWSTR(w.as_ptr()), None, MOVEFILE_DELAY_UNTIL_REBOOT);
    }
}

fn grant_appcontainer_acl(dir: &Path) -> Result<(), String> {
    let dir_str = dir.to_string_lossy().to_string();
    for sid in ["*S-1-15-2-1:(OI)(CI)(RX)", "*S-1-15-2-2:(OI)(CI)(RX)"] {
        let status = Command::new("icacls.exe")
            .args([&dir_str, "/grant", sid, "/T", "/Q"])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| format!("Failed to run icacls.exe: {e}"))?;
        if !status.success() {
            return Err(format!("icacls.exe exited with {}", status));
        }
    }
    Ok(())
}

fn run_regsvr32(dll_path: &Path, unregister: bool) -> Result<(), String> {
    let sys_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    let regsvr = PathBuf::from(sys_root).join("System32").join("regsvr32.exe");
    let mut args = vec!["/s".to_string()];
    if unregister {
        args.push("/u".to_string());
    }
    args.push(dll_path.to_string_lossy().to_string());

    let status = Command::new(regsvr)
        .args(&args)
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("Failed to launch regsvr32.exe: {e}"))?;
    if !status.success() {
        return Err(format!("regsvr32.exe exited with {}", status));
    }
    Ok(())
}

fn write_uninstall_registry(install_root: &Path, uninstaller_path: &Path) -> Result<(), String> {
    unsafe {
        let key_w = to_wide(UNINSTALL_REG_KEY);
        let mut hkey = Default::default();
        let res = RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(key_w.as_ptr()),
            0,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        );
        if res.is_err() {
            return Err("RegCreateKeyExW failed".into());
        }

        let set_sz = |name: &str, val: &str| {
            let n = to_wide(name);
            let v = to_wide(val);
            let bytes = std::slice::from_raw_parts(
                v.as_ptr() as *const u8,
                v.len() * std::mem::size_of::<u16>(),
            );
            let _ = RegSetValueExW(hkey, PCWSTR(n.as_ptr()), 0, REG_SZ, Some(bytes));
        };
        let set_dword = |name: &str, val: u32| {
            let n = to_wide(name);
            let bytes = val.to_le_bytes();
            let _ = RegSetValueExW(hkey, PCWSTR(n.as_ptr()), 0, REG_DWORD, Some(&bytes));
        };

        set_sz("DisplayName", "CantonHK (廣東拼音) Input Method");
        set_sz("DisplayVersion", env!("CARGO_PKG_VERSION"));
        set_sz("Publisher", "CantonHK");
        set_sz("InstallLocation", &install_root.to_string_lossy());
        set_sz(
            "UninstallString",
            &format!("\"{}\" --uninstall", uninstaller_path.to_string_lossy()),
        );
        set_dword("NoModify", 1);
        set_dword("NoRepair", 1);

        let _ = RegCloseKey(hkey);
    }
    Ok(())
}

fn delete_uninstall_registry() {
    unsafe {
        let key_w = to_wide(UNINSTALL_REG_KEY);
        let _ = RegDeleteTreeW(HKEY_LOCAL_MACHINE, PCWSTR(key_w.as_ptr()));
    }
}

fn is_admin() -> bool {
    unsafe { IsUserAnAdmin().as_bool() }
}

fn relaunch_as_admin(action: Action, silent: bool) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut param_str = match action {
        Action::Install => "--install".to_string(),
        Action::Uninstall => "--uninstall".to_string(),
    };
    if silent {
        param_str.push_str(" --silent");
    }
    let exe_w = to_wide(&exe.to_string_lossy());
    let params_w = to_wide(&param_str);
    unsafe {
        let _ = ShellExecuteW(
            HWND::default(),
            w!("runas"),
            PCWSTR(exe_w.as_ptr()),
            PCWSTR(params_w.as_ptr()),
            None,
            SW_SHOWNORMAL,
        );
    }
}

fn msg_box(title: &str, text: &str, flags: u32) -> i32 {
    let title_w = to_wide(title);
    let text_w = to_wide(text);
    unsafe {
        MessageBoxW(
            HWND::default(),
            PCWSTR(text_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE(flags),
        )
        .0
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    OsString::from(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
