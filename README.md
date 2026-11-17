# CantonHK (廣東拼音) — Windows 10/11 Native Input Method Engine

A lightweight, native Windows 10 and Windows 11 Cantonese phonetic Input Method Engine (IME) built on the **Windows Text Services Framework (TSF / `msctf.dll`)** in Rust. Designed to behave like Microsoft's built-in system IMEs across Win32 desktop apps, UWP/WinUI 3 apps, and the Windows 11 Shell.

> [!NOTE]
> **Bring Your Own `.cin` Table:** No `.cin` dictionary file or compiled binary is included in this repository. Place your own `cantonhk.cin` table in the repository root before building.

## Features

- **Native Windows 10/11 TSF Integration (`canton-tsf`):**
  - Registers under Traditional Chinese (`zh-TW`, `0x0404`) with `IMMERSIVESUPPORT` and `SYSTRAYSUPPORT`.
  - Works in Win32 apps (Notepad, Sublime Text, VS Code, browsers), Windows Terminal, UWP/WinUI 3 apps, and Windows 11 Start/Search.
  - Inline composition (`ITfContextComposition`) with dotted underline display attributes.
  - System tray `中` / `英` indicator (`ITfLangBarItemButton`) and bare-`Shift` mode toggle.
- **Fluent Direct2D / DirectWrite Candidate Window (`canton-ui`):**
  - Non-activating (`WS_EX_NOACTIVATE | WS_EX_TOPMOST`) popup anchored to the text caret.
  - Windows 11 rounded corners, automatic Dark/Light theme matching, Per-Monitor DPI scaling, and multi-monitor screen-edge clamping.
- **Fast Zero-Copy Engine & Predictive Completions (`canton-core`):**
  - Compiles `cantonhk.cin` at build time into an embedded static UTF-16LE lexicon (zero runtime disk I/O).
  - Supports exact matches plus predictive prefix completions (e.g., typing `cheun` displays `g 長`, `g 詳`, ...).
  - Microsoft Pinyin-style `Space` to commit the highlighted candidate, `1`–`9`/`0` to select slots 1–10, and `PageDown`/`PageUp`/`=`/`-`/arrows to page and navigate.
- **Portable Self-Contained Installer (`canton-installer`):**
  - Single-file `CantonHK-Setup.exe` embedding the compiled DLL, handling UAC elevation, AppContainer ACLs, `regsvr32` registration, and Windows Settings → Apps uninstall support.

## Repository Structure

```text
├── RFC.md                      # Architecture & Strategic Decision Register
├── Cargo.toml                  # Rust workspace manifest
├── crates/
│   ├── canton-core/            # Pure state machine & build-time .cin compiler
│   ├── canton-ui/              # Direct2D / DirectWrite floating candidate window
│   ├── canton-tsf/             # Windows TSF COM server DLL (canton_ime.dll)
│   ├── canton-cli/             # Standalone CLI simulator for offline testing
│   └── canton-installer/       # Self-contained portable setup/uninstaller (.exe)
└── scripts/
    ├── build-all.ps1           # Builds release DLL, CLI, and CantonHK-Setup.exe
    ├── install-dev.ps1         # Elevated dev install/reinstall script
    └── uninstall-dev.ps1       # Elevated dev uninstall script
```

## Building from Source

### Prerequisites
- Rust (`stable-x86_64-pc-windows-gnu` toolchain)
- A `cantonhk.cin` file placed at the repository root (`.\cantonhk.cin`)

### Build & Test
```powershell
# Run unit tests
cargo +stable-x86_64-pc-windows-gnu test --workspace

# Build release artifacts into .\dist\ (canton_ime.dll, canton-cli.exe, CantonHK-Setup.exe)
.\scripts\build-all.ps1
```

## Installation & UAT

- **Portable Installer:** Run `.\dist\CantonHK-Setup.exe` (supports double-click install, reinstall, and uninstall via Windows Settings → Apps).
- **Developer Scripts (Administrator PowerShell):**
  - Install / Reinstall: `.\scripts\install-dev.ps1`
  - Uninstall: `.\scripts\uninstall-dev.ps1`
- **Offline CLI Simulator:** `.\dist\canton-cli.exe`

Once installed, press **`Win + Space`** and select **`CantonHK (廣東拼音)`** under Traditional Chinese.

## Provenance & Acknowledgments

This engine is designed to compile the classic **CantonHK (港式廣東話)** `.cin` phonetic table:
- **Original Table Compilation:** **Hong Kong Senior Citizens Information World (香港長者資訊天地)** — *CantonHK (港式廣東話)*.
- **Upstream Packaging & Distribution:** **Tung-Han Hsieh (`thhsieh`)** and the **XCIN / `chinese-opendesktop` (`cin-tables`)** project (originally imported into `xcin` CVS on December 9, 2000 under the GNU General Public License).
- **Encoding Conversion:** The Big5-to-UTF-8 conversion of the `cantonhk.cin` table used to build this project was performed by the repository author.

## License

This project is licensed under the **GNU General Public License v2.0 (GPL-2.0)** — see [LICENSE](LICENSE) for details. Because compiled binaries (`canton_ime.dll`, `canton-cli.exe`, `CantonHK-Setup.exe`) embed data compiled from the GPL-licensed `CantonHK.cin` table, any distribution of those binaries is governed by the GPL.
