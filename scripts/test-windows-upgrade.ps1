$ErrorActionPreference = 'Stop'
$version = (Select-String -Path (Join-Path $PSScriptRoot '../Cargo.toml') -Pattern '^version = "([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
if (-not $version) { throw 'Could not read the application version' }
$work = Join-Path $env:RUNNER_TEMP ("metra-upgrade-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
# RunAsUser may rebuild the restarted process's environment from its profile.
# Use the disposable runner's normal location so both launch paths reach it.
$env:METRA_CONFIG_DIR = Join-Path $env:APPDATA 'dev.metra.desktop'
if (Test-Path $env:METRA_CONFIG_DIR) { throw 'Upgrade acceptance requires a clean runner profile' }
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
  if (-not (Get-Process explorer -ErrorAction SilentlyContinue)) { Start-Process explorer.exe }
  $legacy = Join-Path $work 'Metra-0.2.2-setup.exe'
  Invoke-WebRequest 'https://github.com/eyotang/Metra/releases/download/v0.2.2/Metra-0.2.2-windows-x64-setup.exe' -OutFile $legacy
  Run-Installer $legacy
  # The legacy installer may launch Metra asynchronously through RunAsUser.
  # Allow that optional launch to settle before stopping it for the upgrade.
  $deadline = (Get-Date).AddSeconds(30)
  while ((Installed-Processes).Count -eq 0 -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 250 }
  Stop-Installed
  Start-Sleep -Milliseconds 500
  $setup = (Resolve-Path "release-assets/Metra-$version-windows-x64-setup.exe").Path
  # Keep the /S-only contract used by installed v0.4.1 clients, while fetching
  # the retained v0.2.2 release as the legacy installation baseline.
  Run-Installer $setup
  $deadline = (Get-Date).AddSeconds(30)
  while ((Installed-Processes).Count -eq 0 -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 250 }
  if ((Installed-Processes).Count -eq 0) { throw 'Legacy /S update did not restart Metra' }
  $migrationWait = [System.Diagnostics.Stopwatch]::StartNew()
  $db = Join-Path $env:METRA_CONFIG_DIR 'settings.db'
  $reader = Join-Path $work 'read-settings.py'
  @'
import pathlib, sqlite3, sys
try:
    path = pathlib.Path(sys.argv[1])
    row = sqlite3.connect(path).execute("SELECT settings_json FROM app_settings WHERE id=1").fetchone() if path.exists() else None
    print(row[0] if row else "null")
except (sqlite3.Error, OSError):
    print("null")
'@ | Set-Content -Encoding utf8NoBOM $reader
  $saved = $null
  $deadline = (Get-Date).AddSeconds(60)
  do {
    if ((Installed-Processes).Count -eq 0) { throw 'Upgraded application stopped during startup' }
    $saved = & python $reader $db | ConvertFrom-Json
    if ($saved -and $saved.bubblePositionVersion -eq 3) { break }
    Start-Sleep -Milliseconds 500
  } while ((Get-Date) -lt $deadline)
  if (-not $saved -or $saved.bubblePositionVersion -ne 3) { throw 'Upgraded Tauri application did not migrate the expected configuration within 60 seconds' }
  Write-Output "Configuration migrated after $([Math]::Round($migrationWait.Elapsed.TotalSeconds, 1)) seconds of startup waiting"
  Start-Sleep -Seconds 3
  if ((Installed-Processes).Count -eq 0) { throw 'Upgraded application stopped after configuration migration' }
  if ($saved.displayMode -ne 'statusBar' -or $saved.cursorBubbleLabel -ne 'CU' -or $saved.bubblePercentMode -ne 'used' -or ($saved.bubbleVisibleProviders -join ',') -ne 'codex,cursor') {
    throw 'Upgrade did not preserve existing status-bar preferences'
  }
  $installedVersion = (Get-ItemProperty 'HKCU:/Software/Microsoft/Windows/CurrentVersion/Uninstall/Metra').DisplayVersion
  if ($installedVersion -ne $version) { throw "Unexpected installed version: $installedVersion" }
  Write-Output "v0.2.2 -> ${version}: legacy /S restart and status-bar settings preserved"
} finally {
  if (Test-Path $installed) { Write-Output "Installed executable version: $((Get-Item $installed).VersionInfo.ProductVersion)" }
  Installed-Processes | Select-Object ProcessId, ParentProcessId, CreationDate, ExecutablePath | Format-List
  $appLog = Join-Path $env:APPDATA 'metra/metra.log'
  if (Test-Path $appLog) { Get-Content $appLog | Select-String -Pattern 'app.start|desktop.start_failed' }
  Stop-Installed
}
