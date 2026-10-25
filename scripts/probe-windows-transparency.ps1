$ErrorActionPreference = 'Stop'
$results = @{}
foreach ($mode in @('default', 'none', 'no-region', 'plain')) {
  $env:METRA_SMOKE_RENDER_MODE = $mode
  try {
    & "$PSScriptRoot/test-windows-desktop.ps1"
    $results[$mode] = 'passed'
  } catch { $results[$mode] = $_.Exception.Message }
  $screens = 'test-results/native-windows'
  if (Test-Path $screens) { Move-Item $screens "test-results/native-$mode" -Force }
}
Remove-Item Env:METRA_SMOKE_RENDER_MODE
$results | ConvertTo-Json | Set-Content 'test-results/transparency-probes.json'
$results | ConvertTo-Json
if ($results['default'] -ne 'passed') { throw 'Production transparency still fails; inspect the control captures' }
