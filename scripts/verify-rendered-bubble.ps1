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
    $messageStream = [IO.MemoryStream]::new()
    try {
      do {
        $buffer = New-Object byte[] 65536
        $received = $socket.ReceiveAsync(
          [ArraySegment[byte]]::new($buffer),
          $cancellation.Token
        ).GetAwaiter().GetResult()
        if ($received.MessageType -eq [Net.WebSockets.WebSocketMessageType]::Close) {
          throw "The DevTools WebSocket closed before request $id completed"
        }
        if ($received.Count -gt 0) { $messageStream.Write($buffer, 0, $received.Count) }
      } while (-not $received.EndOfMessage)

      $response = [Text.Encoding]::UTF8.GetString($messageStream.ToArray()) | ConvertFrom-Json
      if ($response.id -eq $id) { return $response }
    } finally {
      $messageStream.Dispose()
    }
  }
}

$resolvedExecutable = (Resolve-Path -LiteralPath $Executable).Path
$port = Get-FreeTcpPort
$hadPreviousArguments = Test-Path Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$previousArguments = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$process = $null
try {
  $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$port --remote-allow-origins=*"
  $process = Start-Process -FilePath $resolvedExecutable -PassThru
  $startupTimer = [Diagnostics.Stopwatch]::StartNew()
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
  } while (-not $target -and -not $process.HasExited -and $startupTimer.ElapsedMilliseconds -lt $TimeoutMs)
  if (-not $target) { throw "Bubble WebView was not ready for pid=$($process.Id)" }

  $socket = [Net.WebSockets.ClientWebSocket]::new()
  $socket.Options.SetRequestHeader("Origin", "http://127.0.0.1:$port")
  $remainingMs = [Math]::Max(1, $TimeoutMs - [int]$startupTimer.ElapsedMilliseconds)
  $cancellation = [Threading.CancellationTokenSource]::new($remainingMs)
  try {
    $socket.ConnectAsync([Uri][string]$target.webSocketDebuggerUrl, $cancellation.Token).GetAwaiter().GetResult()
    $requestId = 1
    $renderReady = $false
    do {
      $response = Send-DevToolsRequest $socket $cancellation $requestId "Runtime.evaluate" @{
        expression = "document.readyState === 'complete' && Boolean(document.querySelector('.bubble-shell'))"
        returnByValue = $true
      }
      $requestId++
      $renderReady = $response.result.result.value -eq $true
      if (-not $renderReady) { Start-Sleep -Milliseconds 50 }
    } while (-not $renderReady -and $startupTimer.ElapsedMilliseconds -lt $TimeoutMs)
    if (-not $renderReady) { throw "Bubble DOM was not rendered for pid=$($process.Id)" }

    Send-DevToolsRequest $socket $cancellation $requestId "Runtime.evaluate" @{
      expression = "new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve(true))))"
      awaitPromise = $true
      returnByValue = $true
    } | Out-Null
    $requestId++
    $startupTimer.Stop()

    $response = Send-DevToolsRequest $socket $cancellation $requestId "Page.captureScreenshot" @{ format = "png"; fromSurface = $true }
    $bytes = [Convert]::FromBase64String([string]$response.result.data)
    $stream = [IO.MemoryStream]::new($bytes, $false)
    $bitmap = [Drawing.Bitmap]::new($stream)
    try {
      $changed = 0
      for ($y = 0; $y -lt $bitmap.Height; $y++) {
        for ($x = 0; $x -lt $bitmap.Width; $x++) {
          $pixel = $bitmap.GetPixel($x, $y)
          if ($pixel.A -gt 0 -and ($pixel.R + $pixel.G + $pixel.B) -ge 24) { $changed++ }
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
  if ($process) {
    Stop-Process -InputObject $process -Force -ErrorAction SilentlyContinue
    Wait-Process -InputObject $process -Timeout 2 -ErrorAction SilentlyContinue
  }
  if ($hadPreviousArguments) {
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $previousArguments
  } else {
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
  }
}
