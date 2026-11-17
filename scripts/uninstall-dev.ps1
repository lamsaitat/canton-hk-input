param(
    [string]$InstallDir = "$env:ProgramFiles\CantonHK",
    [switch]$Quiet
)
$ErrorActionPreference = "Continue"

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator
)
if (-not $isAdmin) {
    Write-Error "Please run uninstall-dev.ps1 in an elevated (Administrator) PowerShell window."
    exit 1
}

$DestX64 = Join-Path $InstallDir "x64\canton_ime.dll"
$DestX86 = Join-Path $InstallDir "x86\canton_ime.dll"

if (Test-Path $DestX64) {
    Start-Process -FilePath "$env:SystemRoot\System32\regsvr32.exe" -ArgumentList "/u /s `"$DestX64`"" -Wait | Out-Null
}
if (Test-Path $DestX86) {
    Start-Process -FilePath "$env:SystemRoot\SysWOW64\regsvr32.exe" -ArgumentList "/u /s `"$DestX86`"" -Wait | Out-Null
}

# Remove old/locked DLLs safely (if a host app still holds a file lock, rename it out of the way)
$stamp = Get-Date -Format "yyyyMMddHHmmss"
foreach ($dll in @($DestX64, $DestX86)) {
    if (Test-Path $dll) {
        try {
            Remove-Item -Force $dll -ErrorAction Stop
        }
        catch {
            $oldPath = "$dll.old.$stamp"
            try {
                Rename-Item -Path $dll -NewName (Split-Path -Leaf $oldPath) -ErrorAction Stop
                if (-not $Quiet) {
                    Write-Warning "Locked in memory; renamed $dll -> $oldPath (will clear on sign-out/reboot)."
                }
            }
            catch {
                Write-Warning "Could not remove or rename $dll. Switch away from CantonHK (Win+Space) and retry."
            }
        }
    }
}

# Clean up any leftover .old.* files from earlier cycles
if (Test-Path $InstallDir) {
    Get-ChildItem -Path $InstallDir -Recurse -Filter "*.old.*" -ErrorAction SilentlyContinue | ForEach-Object {
        Remove-Item -Force $_.FullName -ErrorAction SilentlyContinue
    }
    if (-not (Get-ChildItem -Path $InstallDir -Recurse -File -ErrorAction SilentlyContinue)) {
        Remove-Item -Recurse -Force $InstallDir -ErrorAction SilentlyContinue
    }
}

if (-not $Quiet) {
    Write-Host "==> CantonHK IME unregistered and cleaned up." -ForegroundColor Green
}
