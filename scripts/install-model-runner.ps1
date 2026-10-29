[CmdletBinding()]
param(
    [string]$ArchivePath,
    [string]$Prefix = (Join-Path $env:USERPROFILE ".marina"),
    [switch]$Uninstall
)

$ErrorActionPreference = "Stop"

if (-not $Uninstall -and -not (Test-Path -LiteralPath $ArchivePath -PathType Leaf)) {
    throw "Release archive not found: $ArchivePath"
}

$prefixPath = [System.IO.Path]::GetFullPath($Prefix)
$binPath = Join-Path $prefixPath "bin"
$libPath = Join-Path $prefixPath "lib"
if ($Uninstall) {
    Remove-Item -LiteralPath (Join-Path $binPath "talvik.exe") -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath (Join-Path $binPath "talvikd.exe") -Force -ErrorAction SilentlyContinue
    if (Test-Path -LiteralPath $binPath) {
        Get-ChildItem -LiteralPath $binPath -Filter "*.dll" -File -ErrorAction SilentlyContinue |
            Remove-Item -Force
    }
    if (Test-Path -LiteralPath $libPath) {
        Get-ChildItem -LiteralPath $libPath -Filter "*.dll" -File -ErrorAction SilentlyContinue |
            Remove-Item -Force
    }
    Write-Host "Removed Talvik executables and native libraries from $prefixPath; models and state were preserved."
    exit 0
}

$tempPath = Join-Path ([System.IO.Path]::GetTempPath()) ("talvik-install-" + [guid]::NewGuid().ToString("N"))

try {
    New-Item -ItemType Directory -Force -Path $tempPath | Out-Null
    Expand-Archive -LiteralPath $ArchivePath -DestinationPath $tempPath -Force

    $packageRoot = $tempPath
    if (Test-Path -LiteralPath (Join-Path $tempPath "talvik\bin")) {
        $packageRoot = Join-Path $tempPath "talvik"
    }

    $required = @("bin\talvik.exe", "bin\talvikd.exe", "lib")
    foreach ($relativePath in $required) {
        if (-not (Test-Path -LiteralPath (Join-Path $packageRoot $relativePath))) {
            throw "Invalid Talvik archive; missing $relativePath"
        }
    }

    New-Item -ItemType Directory -Force -Path $prefixPath | Out-Null
    # Remove previously installed native DLLs so switching between the CPU and
    # Vulkan packages cannot leave a stale backend (for example ggml-vulkan.dll).
    foreach ($directory in @($binPath, $libPath)) {
        if (Test-Path -LiteralPath $directory) {
            Get-ChildItem -LiteralPath $directory -Filter "*.dll" -File | Remove-Item -Force
        }
    }
    Copy-Item -LiteralPath (Join-Path $packageRoot "bin") -Destination $prefixPath -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $packageRoot "lib") -Destination $prefixPath -Recurse -Force

    # The Vulkan package ships a placement template; never overwrite user settings.
    $configTemplate = Join-Path $packageRoot "config.vulkan.yaml"
    $configPath = Join-Path $prefixPath "config.yaml"
    $vulkanInstalled = Test-Path -LiteralPath (Join-Path $libPath "ggml-vulkan.dll")
    if ((Test-Path -LiteralPath $configTemplate) -and -not (Test-Path -LiteralPath $configPath)) {
        Copy-Item -LiteralPath $configTemplate -Destination $configPath
        Write-Host "Wrote $configPath with Vulkan GPU placement (falls back to CPU if no usable GPU)."
    } elseif ($vulkanInstalled -and (Test-Path -LiteralPath $configPath)) {
        Write-Host "Kept existing $configPath. For GPU use set execution_backend: vulkan and placement_policy: gpu_preferred."
    }

    # Windows searches beside the executable by default. Keep the canonical
    # payload in lib and mirror DLLs into bin so talvikd.exe can start without
    # requiring a global PATH mutation.
    Get-ChildItem -LiteralPath (Join-Path $prefixPath "lib") -File -Filter "*.dll" |
        Copy-Item -Destination (Join-Path $prefixPath "bin") -Force

    foreach ($directory in @("models", "state", "logs")) {
        New-Item -ItemType Directory -Force -Path (Join-Path $prefixPath $directory) | Out-Null
    }

    Write-Host "Installed Talvik for the current Windows user."
    Write-Host "Binaries: $prefixPath\bin"
    Write-Host "Native libraries: $prefixPath\lib"
    Write-Host "Models: $prefixPath\models"
    Write-Host "No administrator privileges or system-wide files were required."
    Write-Host "Add $prefixPath\bin to your user PATH, then run talvik --version."
} finally {
    if (Test-Path -LiteralPath $tempPath) {
        Remove-Item -LiteralPath $tempPath -Recurse -Force
    }
}
