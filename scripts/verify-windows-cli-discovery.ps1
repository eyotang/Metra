param(
  [Parameter(Mandatory = $true)][string]$Executable,
  [int]$TimeoutMs = 8000
)

$ErrorActionPreference = "Stop"
$resolvedExecutable = (Resolve-Path -LiteralPath $Executable).Path
$tempRoot = Join-Path ([IO.Path]::GetTempPath()) ("metra-cli-discovery-" + [guid]::NewGuid().ToString("N"))
$launcher = Join-Path $tempRoot "cursor-agent.ps1"
$log = Join-Path $env:APPDATA "metra\metra.log"
New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null
[IO.File]::WriteAllText($launcher, 'Write-Output ''{"authenticated":true,"plan":"pro"}''', [Text.UTF8Encoding]::new($true))

$savedPath = $env:PATH
$process = $null
$startedAt = [DateTime]::UtcNow.AddSeconds(-1)
try {
  $env:PATH = "$tempRoot;$env:SystemRoot\System32;$env:SystemRoot"
  $process = Start-Process -FilePath $resolvedExecutable -PassThru
  $deadline = [Environment]::TickCount64 + $TimeoutMs
  do {
    if (Test-Path -LiteralPath $log) {
      $found = Get-Content -Encoding UTF8 -LiteralPath $log | Where-Object {
        $_ -match 'cli\.discovery\.found' -and $_ -match 'cursor-agent\.ps1'
      } | Select-Object -Last 1
      if ($found) {
        Write-Output $found
        return
      }
    }
    Start-Sleep -Milliseconds 100
    $process.Refresh()
  } while (-not $process.HasExited -and [Environment]::TickCount64 -lt $deadline)

  $tail = if (Test-Path -LiteralPath $log) { (Get-Content -Encoding UTF8 -LiteralPath $log | Select-Object -Last 30) -join "`n" } else { "log missing" }
  throw "Cursor Agent PowerShell launcher was not discovered.`n$tail"
} finally {
  if ($process -and -not $process.HasExited) {
    Stop-Process -Id $process.Id -Force
    Wait-Process -Id $process.Id -Timeout 2 -ErrorAction SilentlyContinue
  }
  $env:PATH = $savedPath
  if (Test-Path -LiteralPath $tempRoot) {
    $resolvedTemp = (Resolve-Path -LiteralPath $tempRoot).Path
    $systemTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (-not $resolvedTemp.StartsWith($systemTemp, [StringComparison]::OrdinalIgnoreCase)) { throw "Unsafe test cleanup path: $resolvedTemp" }
    $items = @(Get-ChildItem -LiteralPath $resolvedTemp -Force -Recurse)
    if ($items | Where-Object { ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 }) { throw "Refusing to clean test directory containing a reparse point" }
    Remove-Item -LiteralPath $resolvedTemp -Recurse -Force
  }
}