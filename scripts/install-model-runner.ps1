[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ArchivePath,
    [string]$Prefix = (Join-Path $env:USERPROFILE ".marina")
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $ArchivePath -PathType Leaf)) {
    throw "Release archive not found: $ArchivePath"
}

$prefixPath = [System.IO.Path]::GetFullPath($Prefix)
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
    Copy-Item -LiteralPath (Join-Path $packageRoot "bin") -Destination $prefixPath -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $packageRoot "lib") -Destination $prefixPath -Recurse -Force

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
