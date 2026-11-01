[CmdletBinding()]
param(
    [ValidateSet("cpu", "vulkan")]
    [string]$Variant = "cpu",
    [string]$BinDirectory = (Join-Path (Get-Location) "target\release"),
    [string]$NativeLibDir,
    [string]$OutputDirectory = (Join-Path (Get-Location) "target\release")
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$nativeHome = if ($env:TALVIK_HOME) { $env:TALVIK_HOME } else { Join-Path $env:USERPROFILE ".marina" }
if (-not $NativeLibDir) {
    $NativeLibDir = Join-Path $nativeHome $(if ($Variant -eq "vulkan") { "lib-vulkan" } else { "lib" })
}
$archiveName = if ($Variant -eq "vulkan") { "talvik-windows-x86_64-vulkan-preview.zip" } else { "talvik-windows-x86_64-preview.zip" }

foreach ($exe in @("talvik.exe", "talvikd.exe")) {
    if (-not (Test-Path -LiteralPath (Join-Path $BinDirectory $exe))) { throw "Missing $exe in $BinDirectory" }
}
$required = @("llama.dll", "ggml.dll", "ggml-base.dll", "ggml-cpu.dll", "mtmd.dll", "oid-chat-template.dll")
if ($Variant -eq "vulkan") { $required += "ggml-vulkan.dll" }
foreach ($name in $required) {
    if (-not (Test-Path -LiteralPath (Join-Path $NativeLibDir $name))) { throw "Missing $name in $NativeLibDir" }
}
if ($Variant -eq "cpu" -and (Test-Path -LiteralPath (Join-Path $NativeLibDir "ggml-vulkan.dll"))) {
    throw "$NativeLibDir contains ggml-vulkan.dll; refusing to build the CPU package from it."
}

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
$vsPath = & $vswhere -latest -products "*" -property installationPath
if ($LASTEXITCODE -ne 0 -or -not $vsPath) { throw "Visual Studio installation was not found" }
$crtPath = Get-ChildItem -LiteralPath (Join-Path $vsPath "VC\Redist\MSVC") -Directory |
    Where-Object { $_.Name -match '^\d+(\.\d+)+$' } |
    Sort-Object { [version]$_.Name } -Descending |
    ForEach-Object { Get-ChildItem -LiteralPath (Join-Path $_.FullName "x64") -Directory -Filter "Microsoft.VC*.CRT" -ErrorAction SilentlyContinue } |
    Select-Object -First 1
if (-not $crtPath) { throw "x64 Visual C++ runtime redistributable was not found" }

$stage = Join-Path ([System.IO.Path]::GetTempPath()) ("talvik-package-" + [guid]::NewGuid().ToString("N"))
try {
    $package = Join-Path $stage "talvik"
    New-Item -ItemType Directory -Force -Path (Join-Path $package "bin"), (Join-Path $package "lib") | Out-Null
    Copy-Item (Join-Path $BinDirectory "talvik.exe"), (Join-Path $BinDirectory "talvikd.exe") (Join-Path $package "bin")
    Get-ChildItem -LiteralPath $NativeLibDir -File | Where-Object { $_.Extension -in ".dll", ".lib" } |
        Copy-Item -Destination (Join-Path $package "lib")
    Get-ChildItem -LiteralPath $crtPath.FullName -Filter "*.dll" -File | Copy-Item -Destination (Join-Path $package "lib")
    Get-ChildItem -LiteralPath (Join-Path $package "lib") -Filter "*.dll" -File | Copy-Item -Destination (Join-Path $package "bin")
    Copy-Item (Join-Path $root "LICENSE") $package
    Copy-Item (Join-Path $root "scripts\install-model-runner.ps1") $package
    Copy-Item (Join-Path $root "packaging\talvik\README-WINDOWS.txt") $package
    Copy-Item (Join-Path $root "packaging\talvik\SETUP-WINDOWS.cmd") $package
    if ($Variant -eq "vulkan") {
        Copy-Item (Join-Path $root "packaging\talvik\config.vulkan.yaml") $package
    }

    New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
    $archive = Join-Path $OutputDirectory $archiveName
    Remove-Item -LiteralPath $archive, "$archive.sha256" -Force -ErrorAction SilentlyContinue
    Compress-Archive -Path (Join-Path $package "*") -DestinationPath $archive
    (Get-FileHash $archive -Algorithm SHA256).Hash.ToLower() + "  $archiveName" | Out-File "$archive.sha256" -Encoding ascii
    Write-Host "Created $archive"
} finally {
    if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
}
