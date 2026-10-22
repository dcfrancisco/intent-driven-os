[CmdletBinding()]
param(
    [string]$MarinaPath = (Join-Path (Get-Location) "target\release\marina.exe"),
    [string]$BaseUrl = "http://127.0.0.1:11434",
    [string]$Model = $env:MARINA_MODEL,
    [string]$ApiKey = $env:MARINA_API_KEY
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $MarinaPath -PathType Leaf)) {
    throw "Marina executable not found: $MarinaPath"
}
if ([string]::IsNullOrWhiteSpace($Model)) { throw "Set MARINA_MODEL to a discovered GGUF model id." }
if ([string]::IsNullOrWhiteSpace($ApiKey)) { throw "Set MARINA_API_KEY without printing it." }

$version = & $MarinaPath --version
if ($LASTEXITCODE -ne 0) { throw "Marina --version failed." }
Write-Host "version: $version"

$health = Invoke-RestMethod -Uri "$BaseUrl/healthz" -Method Get
if ($health.status -ne "ok") { throw "Health check failed." }
Write-Host "health: ok"

$headers = @{ Authorization = "Bearer $ApiKey" }
$models = Invoke-RestMethod -Uri "$BaseUrl/v1/models" -Method Get -Headers $headers
$modelMatch = @($models.data | Where-Object { $_.id -eq $Model })
if ($modelMatch.Count -ne 1) { throw "Model '$Model' was not found through $BaseUrl/v1/models." }
Write-Host "model: $Model"

$payload = @{
    model = $Model
    request_id = "windows-smoke-$([guid]::NewGuid().ToString('N'))"
    messages = @(@{ role = "user"; content = "Reply with exactly WINDOWS_SMOKE_OK" })
    max_tokens = 8
    context_size = 512
} | ConvertTo-Json -Depth 5

$completion = Invoke-RestMethod -Uri "$BaseUrl/v1/chat/completions" -Method Post -Headers $headers -ContentType "application/json" -Body $payload
if (-not $completion.id) { throw "Completion response did not contain a generation id." }
Write-Host "completion: $($completion.id)"
Write-Host "Windows Marina smoke test: PASS"