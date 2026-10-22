[CmdletBinding()]
param(
    [string]$LlamaRoot = (Join-Path $env:TEMP "talvik-llama.cpp"),
    [string]$LlamaBuild = (Join-Path $env:TEMP "talvik-llama-windows-cpu-build"),
    [string]$NativeLibDir = (Join-Path $(if ($env:TALVIK_HOME) { $env:TALVIK_HOME } elseif ($env:MARINA_HOME) { $env:MARINA_HOME } else { Join-Path $env:USERPROFILE ".marina" }) "lib"),
    [string]$Repository = "https://github.com/ggml-org/llama.cpp.git",
    [string]$Commit = "18443257a30c884d5332abb8e7dc43c7ffe42fda"
)

$ErrorActionPreference = "Stop"

function Require-Command([string]$Name, [string]$Hint) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "$Name is required. $Hint"
    }
}

Require-Command "git" "Install Git and add it to PATH."
Require-Command "cmake" "Install CMake and add it to PATH."
Require-Command "cargo" "Install Rust with rustup and add Cargo to PATH."
Require-Command "rustup" "Install Rust with rustup."
Require-Command "cl" "Run this script from a Visual Studio Developer PowerShell with the C++ workload installed."
Require-Command "link" "Run this script from a Visual Studio Developer PowerShell with the C++ workload installed."
Require-Command "lib" "Run this script from a Visual Studio Developer PowerShell with the C++ workload installed."

$installedTargets = & rustup target list --installed
if ($LASTEXITCODE -ne 0 -or -not ($installedTargets | Select-String -SimpleMatch "x86_64-pc-windows-msvc")) {
    throw "Rust target x86_64-pc-windows-msvc is required. Install it with: rustup target add x86_64-pc-windows-msvc"
}

if (-not (Test-Path -LiteralPath (Join-Path $LlamaRoot ".git"))) {
    & git clone --depth 1 $Repository $LlamaRoot
    if ($LASTEXITCODE -ne 0) { throw "Failed to clone llama.cpp." }
}

& git -C $LlamaRoot fetch --depth 1 origin $Commit
if ($LASTEXITCODE -ne 0) { throw "Failed to fetch llama.cpp revision $Commit." }
& git -C $LlamaRoot checkout --detach $Commit
if ($LASTEXITCODE -ne 0) { throw "Failed to select llama.cpp revision $Commit." }

& cmake -S $LlamaRoot -B $LlamaBuild `
    "-DGGML_NATIVE=OFF" `
    "-DGGML_AVX=OFF" `
    "-DGGML_AVX2=OFF" `
    "-DGGML_AVX512=OFF" `
    "-DGGML_FMA=OFF" `
    "-DGGML_F16C=OFF" `
    "-DGGML_SSE42=OFF" `
    "-DGGML_BMI2=OFF" `
    "-DGGML_OPENMP=OFF" `
    "-DGGML_METAL=OFF" `
    "-DGGML_BLAS=OFF" `
    "-DLLAMA_BUILD_TESTS=OFF" `
    "-DLLAMA_BUILD_EXAMPLES=OFF" `
    "-DLLAMA_CURL=OFF" `
    "-DCMAKE_BUILD_TYPE=Release"
if ($LASTEXITCODE -ne 0) { throw "CMake configuration failed." }

& cmake --build $LlamaBuild --config Release --target llama --parallel 2
if ($LASTEXITCODE -ne 0) { throw "llama.cpp build failed." }

New-Item -ItemType Directory -Force -Path $NativeLibDir | Out-Null
$nativeFiles = Get-ChildItem -Path $LlamaBuild -Recurse -File |
    Where-Object { $_.Name -in @("llama.lib", "llama.dll") -or $_.Name -like "ggml*.lib" -or $_.Name -like "ggml*.dll" }
if (-not ($nativeFiles | Where-Object Name -eq "llama.lib")) { throw "llama.lib was not produced." }
if (-not ($nativeFiles | Where-Object Name -eq "llama.dll")) { throw "llama.dll was not produced." }

foreach ($nativeFile in $nativeFiles) {
    Copy-Item -LiteralPath $nativeFile.FullName -Destination $NativeLibDir -Force
}

Write-Host "Native Windows x86_64 llama.cpp is installed at: $NativeLibDir"
Write-Host "Build Talvik with:"
Write-Host "`$env:LLAMA_CPP_LIB_DIR = '$NativeLibDir'"
Write-Host "`$env:LLAMA_CPP_REQUIRED = '1'"
Write-Host "cargo build --release -p oid-console --bin talvik --bin talvikd"
