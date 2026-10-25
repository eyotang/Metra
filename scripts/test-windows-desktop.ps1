$ErrorActionPreference = 'Stop'
$env:METRA_CONFIG_DIR = Join-Path $env:RUNNER_TEMP ("metra-native-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $env:METRA_CONFIG_DIR | Out-Null
if (-not (Get-Process explorer -ErrorAction SilentlyContinue)) { Start-Process explorer.exe }
$binary = (Resolve-Path 'target/debug/metra.exe').Path
$app = Start-Process -FilePath $binary -ArgumentList '--desktop-smoke-test' -PassThru
try {
  if (-not $app.WaitForExit(60000)) { throw 'Native acceptance application timed out' }
  $reportPath = Join-Path $env:METRA_CONFIG_DIR 'desktop-smoke.json'
  if (-not (Test-Path $reportPath)) { throw "Native application exited without a report (exit $($app.ExitCode))" }
  $report = Get-Content -Raw $reportPath | ConvertFrom-Json
  New-Item -ItemType Directory -Path 'test-results/native-windows' -Force | Out-Null
  Copy-Item $reportPath 'test-results/native-windows/desktop-smoke.json'
  if ($app.ExitCode -ne 0 -or -not $report.ok) { throw "Native acceptance failed: $($report | ConvertTo-Json -Compress)" }
  $report | ConvertTo-Json
} finally {
  if (-not $app.HasExited) { Stop-Process -Id $app.Id -Force }
}
