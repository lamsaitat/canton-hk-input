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
    New-Item -ItemType Directory -Force -Path $DistX64, $DistX86 | Out-Null

    Copy-Item -Force (Join-Path $RepoRoot "target\x86_64-pc-windows-gnu\release\canton_ime.dll") (Join-Path $DistX64 "canton_ime.dll")
    Copy-Item -Force (Join-Path $RepoRoot "target\x86_64-pc-windows-gnu\release\canton-cli.exe") (Join-Path $RepoRoot "dist\canton-cli.exe")

    Write-Host "==> Building x86 release (canton_ime.dll for 32-bit apps / 32-bit Office)..." -ForegroundColor Cyan
    $LlvmMingwBin = Get-ChildItem "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\MartinStorsjo.LLVM-MinGW*" -Directory -ErrorAction SilentlyContinue |
        ForEach-Object { Get-ChildItem $_.FullName -Directory -ErrorAction SilentlyContinue } |
        ForEach-Object { Join-Path $_.FullName "bin" } |
        Where-Object { Test-Path $_ } |
        Select-Object -First 1
    if (-not $LlvmMingwBin) { throw "LLVM-MinGW bin folder not found" }
    $env:CARGO_TARGET_I686_PC_WINDOWS_GNULLVM_LINKER = Join-Path $LlvmMingwBin "i686-w64-mingw32-clang.exe"
    & $Cargo +stable-x86_64-pc-windows-gnu build --release --target i686-pc-windows-gnullvm -p canton-tsf
    if ($LASTEXITCODE -ne 0) { throw "x86 build failed" }
    Copy-Item -Force (Join-Path $RepoRoot "target\i686-pc-windows-gnullvm\release\canton_ime.dll") (Join-Path $DistX86 "canton_ime.dll")

    Write-Host "==> Building self-contained portable installer (CantonHK-Setup.exe)..." -ForegroundColor Cyan
    & $Cargo +stable-x86_64-pc-windows-gnu build --release --target x86_64-pc-windows-gnu -p canton-installer
    if ($LASTEXITCODE -ne 0) { throw "Installer build failed" }
    Copy-Item -Force (Join-Path $RepoRoot "target\x86_64-pc-windows-gnu\release\canton-installer.exe") (Join-Path $RepoRoot "dist\CantonHK-Setup.exe")

    Write-Host "==> Build complete! Artifacts in $RepoRoot\dist" -ForegroundColor Green
}
finally {
    Pop-Location
}
