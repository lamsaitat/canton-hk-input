param(
    [string]$InstallDir = "$env:ProgramFiles\CantonHK"
)
$ErrorActionPreference = "Stop"

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator
)
if (-not $isAdmin) {
    Write-Error "Please run install-dev.ps1 in an elevated (Administrator) PowerShell window."
    exit 1
}

$RepoRoot = Split-Path -Parent $PSScriptRoot
$SrcX64 = Join-Path $RepoRoot "dist\x64\canton_ime.dll"
$SrcX86 = Join-Path $RepoRoot "dist\x86\canton_ime.dll"

if (-not (Test-Path $SrcX64)) {
    Write-Error "Compiled x64 DLL not found in dist\x64\. Run .\scripts\build-all.ps1 first."
    exit 1
}

# Cleanly unregister any previous version first
& (Join-Path $PSScriptRoot "uninstall-dev.ps1") -InstallDir $InstallDir -Quiet

$DestX64Dir = Join-Path $InstallDir "x64"
New-Item -ItemType Directory -Force -Path $DestX64Dir | Out-Null

$DestX64 = Join-Path $DestX64Dir "canton_ime.dll"
Copy-Item -Force $SrcX64 $DestX64

if (Test-Path $SrcX86) {
    $DestX86Dir = Join-Path $InstallDir "x86"
    New-Item -ItemType Directory -Force -Path $DestX86Dir | Out-Null
    $DestX86 = Join-Path $DestX86Dir "canton_ime.dll"
    Copy-Item -Force $SrcX86 $DestX86
}

# Grant Read & Execute to AppContainer / UWP / Windows 11 Shell sandboxes
& icacls.exe $InstallDir /grant "*S-1-15-2-1:(OI)(CI)(RX)" /T /Q | Out-Null
& icacls.exe $InstallDir /grant "*S-1-15-2-2:(OI)(CI)(RX)" /T /Q | Out-Null

# Register 64-bit (and optional 32-bit) TSF COM servers
$p64 = Start-Process -FilePath "$env:SystemRoot\System32\regsvr32.exe" -ArgumentList "/s `"$DestX64`"" -Wait -PassThru
if ($p64.ExitCode -ne 0) {
    Write-Error "64-bit regsvr32 failed with exit code $($p64.ExitCode)"
    exit 1
}

if (Test-Path $SrcX86) {
    $p86 = Start-Process -FilePath "$env:SystemRoot\SysWOW64\regsvr32.exe" -ArgumentList "/s `"$DestX86`"" -Wait -PassThru
    if ($p86.ExitCode -ne 0) {
        Write-Error "32-bit regsvr32 failed with exit code $($p86.ExitCode)"
        exit 1
    }
}

Write-Host "==> CantonHK IME installed and registered!" -ForegroundColor Green
Write-Host "    Press Win+Space to switch to 'CantonHK (廣東拼音)' under Traditional Chinese (zh-TW)." -ForegroundColor Cyan
