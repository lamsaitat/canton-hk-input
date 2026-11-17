$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot
$Cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"

Push-Location $RepoRoot
try {
    Write-Host "==> Building x64 release (canton_ime.dll + canton-cli.exe)..." -ForegroundColor Cyan
    & $Cargo +stable-x86_64-pc-windows-gnu build --release --target x86_64-pc-windows-gnu -p canton-tsf -p canton-cli
    if ($LASTEXITCODE -ne 0) { throw "x64 build failed" }

    $DistX64 = Join-Path $RepoRoot "dist\x64"
    $DistX86 = Join-Path $RepoRoot "dist\x86"
    New-Item -ItemType Directory -Force -Path $DistX64 | Out-Null

    Copy-Item -Force (Join-Path $RepoRoot "target\x86_64-pc-windows-gnu\release\canton_ime.dll") (Join-Path $DistX64 "canton_ime.dll")
    Copy-Item -Force (Join-Path $RepoRoot "target\x86_64-pc-windows-gnu\release\canton-cli.exe") (Join-Path $RepoRoot "dist\canton-cli.exe")

    Write-Host "==> Building self-contained portable installer (CantonHK-Setup.exe)..." -ForegroundColor Cyan
    & $Cargo +stable-x86_64-pc-windows-gnu build --release --target x86_64-pc-windows-gnu -p canton-installer
    if ($LASTEXITCODE -ne 0) { throw "Installer build failed" }
    Copy-Item -Force (Join-Path $RepoRoot "target\x86_64-pc-windows-gnu\release\canton-installer.exe") (Join-Path $RepoRoot "dist\CantonHK-Setup.exe")

    if (Get-Command "i686-w64-mingw32-gcc" -ErrorAction SilentlyContinue) {
        Write-Host "==> Building x86 release (canton_ime.dll for 32-bit apps)..." -ForegroundColor Cyan
        & $Cargo +stable-x86_64-pc-windows-gnu build --release --target i686-pc-windows-gnu -p canton-tsf
        if ($LASTEXITCODE -ne 0) { throw "x86 build failed" }
        New-Item -ItemType Directory -Force -Path $DistX86 | Out-Null
        Copy-Item -Force (Join-Path $RepoRoot "target\i686-pc-windows-gnu\release\canton_ime.dll") (Join-Path $DistX86 "canton_ime.dll")
    }
    else {
        Write-Host "==> Skipping optional 32-bit (x86) DLL (i686-w64-mingw32-gcc not found); x64 DLL ready." -ForegroundColor Yellow
    }

    Write-Host "==> Build complete! Artifacts in $RepoRoot\dist" -ForegroundColor Green
}
finally {
    Pop-Location
}
