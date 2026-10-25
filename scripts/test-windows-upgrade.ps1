$ErrorActionPreference = 'Stop'
$version = (Select-String -Path (Join-Path $PSScriptRoot '../Cargo.toml') -Pattern '^version = "([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
if (-not $version) { throw 'Could not read the application version' }
$work = Join-Path $env:RUNNER_TEMP ("metra-upgrade-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
$env:METRA_CONFIG_DIR = Join-Path $work 'config'
New-Item -ItemType Directory -Path $env:METRA_CONFIG_DIR | Out-Null
@{ displayMode='statusBar'; bubbleProviderOrder=@('codex','cursor','claude'); bubbleVisibleProviders=@('codex','cursor'); bubblePercentMode='used'; cursorBubbleLabel='CU'; autostart=$false } |
  ConvertTo-Json | Set-Content -Encoding utf8NoBOM (Join-Path $env:METRA_CONFIG_DIR 'settings.json')
$installed = Join-Path $env:LOCALAPPDATA 'Metra/metra.exe'
function Installed-Processes {
  @(Get-CimInstance Win32_Process -Filter "Name='metra.exe'" | Where-Object { $_.ExecutablePath -eq $installed })
}
function Run-Installer([string]$path) {
  $process = Start-Process $path -ArgumentList '/S' -PassThru
  if (-not $process.WaitForExit(120000)) {
    Stop-Process -Id $process.Id -Force
    throw "Installer timed out: $path"
  }
  if ($process.ExitCode -ne 0) { throw "Installer failed ($($process.ExitCode)): $path" }
}
function Stop-Installed {
  Installed-Processes | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
}
try {
  $legacy = Join-Path $work 'Metra-0.4.1-setup.exe'
  Invoke-WebRequest 'https://github.com/eyotang/Metra/releases/download/v0.4.1/Metra-0.4.1-windows-x64-setup.exe' -OutFile $legacy
  Run-Installer $legacy
  Stop-Installed
  Start-Sleep -Milliseconds 500
  $setup = (Resolve-Path "release-assets/Metra-$version-windows-x64-setup.exe").Path
  # This is exactly what the released v0.4.1 updater invokes: /S, without /R.
  Run-Installer $setup
  $deadline = (Get-Date).AddSeconds(30)
  while ((Installed-Processes).Count -eq 0 -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 250 }
  if ((Installed-Processes).Count -eq 0) { throw 'Legacy /S update did not restart Metra' }
  Start-Sleep -Seconds 3
  if ((Installed-Processes).Count -eq 0) { throw 'Upgraded application stopped during startup' }
  $db = Join-Path $env:METRA_CONFIG_DIR 'settings.db'
  $saved = & python -c 'import sqlite3,sys; print(sqlite3.connect(sys.argv[1]).execute("SELECT settings_json FROM app_settings WHERE id=1").fetchone()[0])' $db | ConvertFrom-Json
  if ($saved.displayMode -ne 'statusBar' -or $saved.cursorBubbleLabel -ne 'CU' -or $saved.bubblePercentMode -ne 'used' -or ($saved.bubbleVisibleProviders -join ',') -ne 'codex,cursor') {
    throw 'Upgrade did not preserve existing status-bar preferences'
  }
  $installedVersion = (Get-ItemProperty 'HKCU:/Software/Microsoft/Windows/CurrentVersion/Uninstall/Metra').DisplayVersion
  if ($installedVersion -ne $version) { throw "Unexpected installed version: $installedVersion" }
  Write-Output "v0.4.1 -> ${version}: legacy /S restart and status-bar settings preserved"
} finally { Stop-Installed }
