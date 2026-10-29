[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Model,
    [Parameter(Mandatory = $true)][string]$SecondModel
)

$ErrorActionPreference = "Stop"
if ($env:CONFIRM_TALVIK_GPU_QUALIFICATION -ne "YES") {
    throw "Refusing to run. Set CONFIRM_TALVIK_GPU_QUALIFICATION=YES on supported GPU hardware."
}
if (-not (Get-Command talvik -ErrorAction SilentlyContinue)) {
    throw "talvik is required on PATH."
}

& talvik --json status
& talvik --json models
& talvik --json load $Model
& talvik run $Model "Reply with exactly TALVIK_GPU_OK"
& talvik --json load $SecondModel
& talvik --json status
& talvik --json unload $SecondModel
& talvik --json unload $Model

Write-Host "Lifecycle sequence complete. Review daemon diagnostics for selected backend, requested/actual offload, memory, fallback, cancellation, and timing evidence."
