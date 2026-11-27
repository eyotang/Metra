param([ValidateSet('0.2.2', '0.5.18', '0.6.0')][string]$BaselineVersion = '0.2.2')

$ErrorActionPreference = 'Stop'
$version = (Select-String -Path (Join-Path $PSScriptRoot '../Cargo.toml') -Pattern '^version = "([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
if (-not $version) { throw 'Could not read the application version' }
$work = Join-Path $env:RUNNER_TEMP ("metra-upgrade-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
# RunAsUser may rebuild the restarted process's environment from its profile.
# Use the disposable runner's normal location so both launch paths reach it.
$env:METRA_CONFIG_DIR = Join-Path $env:APPDATA 'dev.metra.desktop'
$installed = Join-Path $env:LOCALAPPDATA 'Metra/metra.exe'
if ((Test-Path $env:METRA_CONFIG_DIR) -or (Test-Path (Split-Path $installed))) {
  throw 'Upgrade acceptance requires a clean runner profile and install directory'
}
New-Item -ItemType Directory -Path $env:METRA_CONFIG_DIR | Out-Null
@{ displayMode='statusBar'; bubbleProviderOrder=@('codex','cursor','claude'); bubbleVisibleProviders=@('codex','cursor'); bubblePercentMode='used'; cursorBubbleLabel='CU'; autostart=$false; uiLanguage='ja'; refreshMinutes=15; codexBubbleColor='#ff6b6b' } |
  ConvertTo-Json | Set-Content -Encoding utf8NoBOM (Join-Path $env:METRA_CONFIG_DIR 'settings.json')
function Installed-Processes {
  @(Get-CimInstance Win32_Process -Filter "Name='metra.exe'" | Where-Object { $_.ExecutablePath -eq $installed })
}
function Run-Installer([string]$path, [string[]]$installerArgs = @('/S')) {
  $process = Start-Process $path -ArgumentList $installerArgs -PassThru
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
  $releaseName = "Metra-$BaselineVersion-windows-x64-setup.exe"
  $legacy = Join-Path $work $releaseName
  Invoke-WebRequest "https://github.com/eyotang/Metra/releases/download/v$BaselineVersion/$releaseName" -OutFile $legacy
  Run-Installer $legacy
  $uninstallEntry = 'HKCU:/Software/Microsoft/Windows/CurrentVersion/Uninstall/Metra'
  if ((Get-ItemProperty $uninstallEntry).DisplayVersion -ne $BaselineVersion) { throw 'Baseline installation version mismatch' }
  # The legacy installer may launch Metra asynchronously through RunAsUser.
  # Allow that optional launch to settle before stopping it for the upgrade.
  $deadline = (Get-Date).AddSeconds(30)
  while ((Installed-Processes).Count -eq 0 -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 250 }
  if ($BaselineVersion -ne '0.2.2' -and (Installed-Processes).Count -eq 0) { throw 'Published version did not start' }
  Stop-Installed
  Start-Sleep -Milliseconds 500
  $setup = (Resolve-Path "release-assets/Metra-$version-windows-x64-setup.exe").Path
  if ($BaselineVersion -ne '0.2.2') {
    $portable = (Resolve-Path "release-assets/Metra-$version-windows-x64-portable.exe").Path
    node (Join-Path $PSScriptRoot 'test-packaged-settings.mjs') $installed $portable
    if ($LASTEXITCODE -ne 0) { throw 'Published SQLite settings or candidate portable startup failed' }
  }
  $desktopShortcut = Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) 'Metra.lnk'
  $installerArgs = switch ($BaselineVersion) {
    '0.2.2' { '/S' }
    '0.5.18' { '/S', '/R', '/UPDATE' }
    '0.6.0' { '/S', '/NS' }
  }
  if ($BaselineVersion -eq '0.6.0') { Remove-Item -LiteralPath $desktopShortcut -ErrorAction SilentlyContinue }
  Run-Installer $setup $installerArgs
  $deadline = (Get-Date).AddSeconds(30)
  while ((Installed-Processes).Count -eq 0 -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 250 }
  if ((Installed-Processes).Count -eq 0) { throw 'Silent /S update did not restart Metra' }
  if ($BaselineVersion -eq '0.6.0' -and (Test-Path -LiteralPath $desktopShortcut)) { throw 'Silent update recreated the desktop shortcut' }
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
  if (-not $saved -or $saved.bubblePositionVersion -ne 3) { throw 'Upgraded application did not migrate the expected configuration within 60 seconds' }
  Write-Output "Configuration migrated after $([Math]::Round($migrationWait.Elapsed.TotalSeconds, 1)) seconds of startup waiting"
  Start-Sleep -Seconds 3
  if ((Installed-Processes).Count -eq 0) { throw 'Upgraded application stopped after configuration migration' }
  if ($saved.displayMode -ne 'statusBar' -or $saved.cursorBubbleLabel -ne 'CU' -or $saved.bubblePercentMode -ne 'used' -or ($saved.bubbleVisibleProviders -join ',') -ne 'codex,cursor') {
    throw 'Upgrade did not preserve existing status-bar preferences'
  }
  if ($BaselineVersion -ne '0.2.2' -and ($saved.uiLanguage -ne 'ja' -or $saved.refreshMinutes -ne 15 -or $saved.codexBubbleColor -ne '#ff6b6b')) {
    throw 'Upgrade did not preserve locale, refresh interval and provider color'
  }
  $installedVersion = (Get-ItemProperty $uninstallEntry).DisplayVersion
  if ($installedVersion -ne $version) { throw "Unexpected installed version: $installedVersion" }
  Write-Output "v$BaselineVersion -> ${version}: silent restart and status-bar settings preserved ($installerArgs)"
} finally {
  if (Test-Path $installed) { Write-Output "Installed executable version: $((Get-Item $installed).VersionInfo.ProductVersion)" }
  Installed-Processes | Select-Object ProcessId, ParentProcessId, CreationDate, ExecutablePath | Format-List
  $appLog = Join-Path $env:APPDATA 'metra/metra.log'
  if (Test-Path $appLog) { Get-Content $appLog | Select-String -Pattern 'app.start|desktop.start_failed' }
  Stop-Installed
  $uninstaller = Join-Path (Split-Path $installed) 'uninstall.exe'
  if (Test-Path $uninstaller) {
    Run-Installer $uninstaller
    $deadline = (Get-Date).AddSeconds(30)
    while ((Test-Path (Split-Path $installed)) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 250 }
    if (Test-Path (Split-Path $installed)) { throw 'Fixture installation was not removed' }
  }
  Remove-Item -LiteralPath $env:METRA_CONFIG_DIR -Recurse -Force
}
