$ErrorActionPreference = "Stop"

$workspace = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$tauri = Join-Path $workspace "node_modules\.bin\tauri.cmd"
if (-not (Test-Path -LiteralPath $tauri -PathType Leaf)) {
  throw "Tauri CLI is missing. Run npm install first."
}

Push-Location $workspace
try {
  & $tauri build --no-bundle
  if ($LASTEXITCODE -ne 0) { throw "Tauri portable build failed with exit code $LASTEXITCODE" }

  $config = Get-Content -Raw -Encoding UTF8 "src-tauri\tauri.conf.json" | ConvertFrom-Json
  $source = (Resolve-Path "src-tauri\target\release\metra.exe").Path
  $portable = Join-Path (Split-Path $source) "Metra-$($config.version)-portable.exe"
  Copy-Item -LiteralPath $source -Destination $portable -Force

  & "$PSScriptRoot\verify-rendered-bubble.ps1" -Executable $portable
  if ($LASTEXITCODE -ne 0) { throw "Portable executable did not render its bubble" }
  Write-Output "portable=$portable"
} finally {
  Pop-Location
}