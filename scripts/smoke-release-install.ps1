[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ArchivePath
)

$ErrorActionPreference = "Stop"
$work = Join-Path ([System.IO.Path]::GetTempPath()) ("talvik-release-smoke-" + [guid]::NewGuid().ToString("N"))
$prefix = Join-Path $work "prefix"
$state = Join-Path $work "state"
try {
    New-Item -ItemType Directory -Force -Path $work, $state | Out-Null
    $payload = Join-Path $work "payload"
    Expand-Archive -LiteralPath $ArchivePath -DestinationPath $payload
    $installer = Get-ChildItem -LiteralPath $payload -Filter "install-model-runner.ps1" -Recurse -File |
        Select-Object -First 1 -ExpandProperty FullName
    if (-not $installer) { throw "release payload does not contain install-model-runner.ps1" }
    & $installer -ArchivePath $ArchivePath -Prefix $prefix
    & (Join-Path $prefix "bin\talvik.exe") --help | Out-Null
    & (Join-Path $prefix "bin\talvik.exe") --version | Out-Null
    & (Join-Path $prefix "bin\talvikd.exe") --help | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $prefix "models") | Out-Null
    & $installer -Prefix $prefix -Uninstall
    if ((Test-Path (Join-Path $prefix "bin\talvik.exe")) -or
        -not (Test-Path (Join-Path $prefix "models"))) {
        throw "uninstall smoke check failed"
    }
    Write-Host "Talvik Windows release install smoke: PASS"
} finally {
    if (Test-Path $work) { Remove-Item -LiteralPath $work -Recurse -Force }
}
