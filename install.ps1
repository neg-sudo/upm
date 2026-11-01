# upm installer — Windows (PowerShell)
# Usage: irm https://raw.githubusercontent.com/neg-sudo/upm/main/install.ps1 | iex
# Env: UPM_VERSION, UPM_INSTALL_DIR, UPM_TARGET

[CmdletBinding()]
param(
    [string]$Version = $(if ($env:UPM_VERSION) { $env:UPM_VERSION } else { "latest" }),
    [string]$InstallDir = $(if ($env:UPM_INSTALL_DIR) { $env:UPM_INSTALL_DIR } else { "$env:USERPROFILE\.local\bin" })
)

$ErrorActionPreference = "Stop"
$Repo = "neg-sudo/upm"

function Write-Info($Message) { Write-Host "ℹ $Message" -ForegroundColor Cyan }
function Write-Ok($Message) { Write-Host "✓ $Message" -ForegroundColor Green }
function Write-Warn($Message) { Write-Host "⚠ $Message" -ForegroundColor Yellow }
function Fail($Message) { Write-Host "✗ $Message" -ForegroundColor Red; exit 1 }

$Arch = $env:PROCESSOR_ARCHITECTURE
switch ($Arch) {
    "AMD64" { $ArchPart = "x86_64" }
    "ARM64" { $ArchPart = "aarch64" }
    default { Fail "unsupported architecture: $Arch" }
}

$Target = if ($env:UPM_TARGET) { $env:UPM_TARGET } else { "$ArchPart-pc-windows-msvc" }
$Asset = "upm-$Target.zip"

if ($Version -eq "latest") {
    $Base = "https://github.com/$Repo/releases/latest/download"
} else {
    $Tag = if ($Version.StartsWith("v")) { $Version } else { "v$Version" }
    $Base = "https://github.com/$Repo/releases/download/$Tag"
}

Write-Info "Installing upm $Version for $Target"

$Temp = Join-Path $env:TEMP ("upm-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $Temp | Out-Null

try {
    $Zip = Join-Path $Temp $Asset
    try {
        Invoke-WebRequest -Uri "$Base/$Asset" -OutFile $Zip -UseBasicParsing
    } catch {
        Write-Warn "could not download a prebuilt binary"
        if (Get-Command cargo -ErrorAction SilentlyContinue) {
            Write-Info "building from source with cargo"
            cargo install --git "https://github.com/$Repo" --locked --root "$env:USERPROFILE\.upm"
            New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
            Copy-Item "$env:USERPROFILE\.upm\bin\upm.exe" (Join-Path $InstallDir "upm.exe") -Force
            Write-Ok "installed $(Join-Path $InstallDir 'upm.exe')"
            return
        }
        Fail "no prebuilt binary for $Target and cargo is not installed"
    }

    # Optional checksum verification.
    try {
        $Sums = Join-Path $Temp "SHA256SUMS"
        Invoke-WebRequest -Uri "$Base/SHA256SUMS" -OutFile $Sums -UseBasicParsing
        $Line = Get-Content $Sums | Where-Object { $_ -match [regex]::Escape($Asset) } | Select-Object -First 1
        if ($Line) {
            $Expected = ($Line -split '\s+')[0].ToLowerInvariant()
            $Actual = (Get-FileHash $Zip -Algorithm SHA256).Hash.ToLowerInvariant()
            if ($Expected -ne $Actual) { Fail "checksum mismatch for $Asset" }
            Write-Ok "checksum verified"
        }
    } catch {
        if ($_.Exception.Message -like "*checksum mismatch*") { throw }
    }

    Expand-Archive -Path $Zip -DestinationPath $Temp -Force
    $Binary = Get-ChildItem -Path $Temp -Filter "upm.exe" -Recurse | Select-Object -First 1
    if (-not $Binary) { Fail "archive did not contain upm.exe" }

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item $Binary.FullName (Join-Path $InstallDir "upm.exe") -Force
    Write-Ok "installed $(Join-Path $InstallDir 'upm.exe')"

    $UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if (($UserPath -split ';') -notcontains $InstallDir) {
        [Environment]::SetEnvironmentVariable("PATH", "$InstallDir;$UserPath", "User")
        Write-Warn "$InstallDir was added to your user PATH; open a new terminal"
    }
} finally {
    Remove-Item -Recurse -Force $Temp -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Ok "upm is ready"
Write-Host "  Run 'upm doctor' to get started"
