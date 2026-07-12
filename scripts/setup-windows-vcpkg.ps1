[CmdletBinding()]
param(
    [string]$VcpkgRoot = "",
    [string]$Triplet = ""
)

$ErrorActionPreference = "Stop"

# Resolve the vcpkg triplet from the host architecture when the caller omits it.
# windows-11-arm runners report ARM64 and need arm64-windows; x64 stays x64-windows.
if ([string]::IsNullOrWhiteSpace($Triplet)) {
    if ($env:PROCESSOR_ARCHITECTURE -ieq "ARM64") {
        $Triplet = "arm64-windows"
    } else {
        $Triplet = "x64-windows"
    }
}

$projectRoot = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($VcpkgRoot)) {
    $VcpkgRoot = Join-Path $projectRoot ".cache\vcpkg"
}

$vcpkgRootPath = [System.IO.Path]::GetFullPath($VcpkgRoot)
$vcpkgExe = Join-Path $vcpkgRootPath "vcpkg.exe"
$vcpkgBinDir = Join-Path $vcpkgRootPath "installed\$Triplet\bin"

if (-not (Test-Path $vcpkgRootPath)) {
    New-Item -ItemType Directory -Path (Split-Path -Parent $vcpkgRootPath) -Force | Out-Null
    git clone https://github.com/microsoft/vcpkg $vcpkgRootPath
}

if (-not (Test-Path $vcpkgExe)) {
    & (Join-Path $vcpkgRootPath "bootstrap-vcpkg.bat") -disableMetrics
}

& $vcpkgExe install ffmpeg --triplet $Triplet

$env:VCPKG_ROOT = $vcpkgRootPath
$env:VCPKGRS_DYNAMIC = "1"
if (Test-Path $vcpkgBinDir) {
    $env:PATH = "$vcpkgBinDir;$env:PATH"
}

if ($env:GITHUB_ENV) {
    "VCPKG_ROOT=$vcpkgRootPath" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append
    "VCPKGRS_DYNAMIC=1" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append
}

if ($env:GITHUB_PATH -and (Test-Path $vcpkgBinDir)) {
    $vcpkgBinDir | Out-File -FilePath $env:GITHUB_PATH -Encoding utf8 -Append
}

Write-Host "Windows FFmpeg dependencies are ready via vcpkg."
Write-Host "VCPKG_ROOT=$vcpkgRootPath"
if (Test-Path $vcpkgBinDir) {
    Write-Host "PATH includes $vcpkgBinDir for this shell."
}
Write-Host ""
Write-Host "Next commands:"
Write-Host "  `$env:VCPKG_ROOT = '$vcpkgRootPath'"
Write-Host "  `$env:VCPKGRS_DYNAMIC = '1'"
Write-Host "  cargo build"
