param(
  [Parameter(Mandatory = $true)][string]$Executable,
  [int]$TimeoutMs = 12000,
  [int]$MinimumChangedPixels = 300
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

function Get-FreeTcpPort {
  $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
  $listener.Start()
  try { return ([Net.IPEndPoint]$listener.LocalEndpoint).Port }
  finally { $listener.Stop() }
}

function Send-DevToolsRequest($socket, $cancellation, [int]$id, [string]$method, $params) {
  $message = @{ id = $id; method = $method; params = $params } | ConvertTo-Json -Compress -Depth 6
  $bytes = [Text.Encoding]::UTF8.GetBytes($message)
  $socket.SendAsync(
    [ArraySegment[byte]]::new($bytes),
    [Net.WebSockets.WebSocketMessageType]::Text,
    $true,
    $cancellation.Token
  ).GetAwaiter().GetResult()

  while ($true) {
    $buffer = New-Object byte[] 262144
    $received = $socket.ReceiveAsync(
      [ArraySegment[byte]]::new($buffer),
      $cancellation.Token
    ).GetAwaiter().GetResult()
    $response = [Text.Encoding]::UTF8.GetString($buffer, 0, $received.Count) | ConvertFrom-Json
    if ($response.id -eq $id) { return $response }
  }
}

$resolvedExecutable = (Resolve-Path -LiteralPath $Executable).Path
$port = Get-FreeTcpPort
$previousArguments = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$port --remote-allow-origins=*"
$process = Start-Process -FilePath $resolvedExecutable -PassThru
try {
  $deadline = [Environment]::TickCount64 + $TimeoutMs
  $target = $null
  do {
    Start-Sleep -Milliseconds 100
    try {
      $pages = Invoke-RestMethod -Uri "http://127.0.0.1:$port/json" -TimeoutSec 1
      foreach ($page in $pages) {
        if ([string]$page.url -match "view=bubble") { $target = $page; break }
      }
    } catch {}
    $process.Refresh()
  } while (-not $target -and -not $process.HasExited -and [Environment]::TickCount64 -lt $deadline)
  if (-not $target) { throw "Bubble WebView was not ready for pid=$($process.Id)" }

  $socket = [Net.WebSockets.ClientWebSocket]::new()
  $socket.Options.SetRequestHeader("Origin", "http://127.0.0.1:$port")
  $cancellation = [Threading.CancellationTokenSource]::new($TimeoutMs)
  try {
    $socket.ConnectAsync([Uri][string]$target.webSocketDebuggerUrl, $cancellation.Token).GetAwaiter().GetResult()
    $response = Send-DevToolsRequest $socket $cancellation 1 "Page.captureScreenshot" @{ format = "png"; fromSurface = $true }
    $bytes = [Convert]::FromBase64String([string]$response.result.data)
    $stream = [IO.MemoryStream]::new($bytes, $false)
    $bitmap = [Drawing.Bitmap]::new($stream)
    try {
      $changed = 0
      for ($y = 0; $y -lt $bitmap.Height; $y++) {
        for ($x = 0; $x -lt $bitmap.Width; $x++) {
          $pixel = $bitmap.GetPixel($x, $y)
          if (($pixel.R + $pixel.G + $pixel.B) -ge 24) { $changed++ }
        }
      }
    } finally {
      $bitmap.Dispose()
      $stream.Dispose()
    }
  } finally {
    $socket.Dispose()
    $cancellation.Dispose()
  }
  if ($changed -lt $MinimumChangedPixels) { throw "Bubble WebView is transparent or blank: changed_pixels=$changed" }
  Write-Output "rendered-bubble pid=$($process.Id) changed_pixels=$changed"
} finally {
  if (-not $process.HasExited) {
    Stop-Process -Id $process.Id -Force
    Wait-Process -Id $process.Id -Timeout 2 -ErrorAction SilentlyContinue
  }
  $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $previousArguments
}
