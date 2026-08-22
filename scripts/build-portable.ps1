$ErrorActionPreference = "Stop"

$workspace = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$tauri = Join-Path $workspace "node_modules\.bin\tauri.cmd"
if (-not (Test-Path -LiteralPath $tauri -PathType Leaf)) {
  throw "Tauri CLI is missing. Run pnpm install --frozen-lockfile first."
}

function Assert-MetraNotRunning {
  $runningMetra = @(
    Get-Process -ErrorAction SilentlyContinue |
      Where-Object {
        $_.ProcessName -ieq "metra" -or $_.ProcessName -like "Metra*-portable"
      }
  )
  if ($runningMetra.Count -gt 0) {
    $processNames = ($runningMetra | Select-Object -ExpandProperty ProcessName -Unique) -join ", "
    throw "Close all running Metra instances before building the portable executable. Running: $processNames"
  }
}

Assert-MetraNotRunning

$rustcVersion = & rustc -vV
if ($LASTEXITCODE -ne 0) { throw "Could not inspect the active Rust compiler (exit code $LASTEXITCODE)" }
$hostLine = $rustcVersion | Where-Object { $_ -match '^host:\s+(.+)$' } | Select-Object -First 1
if (-not $hostLine) { throw "Could not determine the native Rust target from rustc -vV" }
$hostTarget = [regex]::Match([string]$hostLine, '^host:\s+(.+)$').Groups[1].Value.Trim()
$targetDirectory = Join-Path $workspace "src-tauri\target"
$portableDirectory = Join-Path $targetDirectory "release"
$hadCargoTargetDir = Test-Path Env:CARGO_TARGET_DIR
$previousCargoTargetDir = $env:CARGO_TARGET_DIR
$env:CARGO_TARGET_DIR = $targetDirectory

Push-Location $workspace
try {
  & $tauri build --no-bundle --target $hostTarget
  if ($LASTEXITCODE -ne 0) { throw "Tauri portable build failed with exit code $LASTEXITCODE" }

  $config = Get-Content -Raw -Encoding UTF8 "src-tauri\tauri.conf.json" | ConvertFrom-Json
  $source = (Resolve-Path (Join-Path $targetDirectory "$hostTarget\release\metra.exe")).Path
  New-Item -ItemType Directory -Path $portableDirectory -Force | Out-Null
  $portable = Join-Path $portableDirectory "Metra-$($config.version)-portable.exe"

  Assert-MetraNotRunning
  Copy-Item -LiteralPath $source -Destination $portable -Force

  Write-Output "portable=$portable"
} finally {
  Pop-Location
  if ($hadCargoTargetDir) {
    $env:CARGO_TARGET_DIR = $previousCargoTargetDir
  } else {
    Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
  }
}
