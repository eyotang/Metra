param(
  [Parameter(Mandatory = $true)][string]$Executable,
  [int]$TimeoutMs = 12000,
  [int]$MinimumChangedPixels = 300
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Threading;

public sealed class MetraWindowInfo
{
    public IntPtr Handle;
    public int Left;
    public int Top;
    public int Right;
    public int Bottom;

    public int Width { get { return Right - Left; } }
    public int Height { get { return Bottom - Top; } }
}

public static class MetraWindowSmokeNative
{
    private const int DwmwaExtendedFrameBounds = 9;
    private const int DwmwaCloaked = 14;
    private const uint GaRoot = 2;
    private const uint InputMouse = 0;
    private const uint MouseEventLeftDown = 0x0002;
    private const uint MouseEventLeftUp = 0x0004;

    private delegate bool EnumWindowsCallback(IntPtr window, IntPtr state);

    [StructLayout(LayoutKind.Sequential)]
    private struct NativeRect
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct NativePoint
    {
        public int X;
        public int Y;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct MouseInput
    {
        public int X;
        public int Y;
        public uint MouseData;
        public uint Flags;
        public uint Time;
        public UIntPtr ExtraInfo;
    }

    [StructLayout(LayoutKind.Explicit)]
    private struct InputUnion
    {
        [FieldOffset(0)]
        public MouseInput Mouse;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct NativeInput
    {
        public uint Type;
        public InputUnion Data;
    }

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsCallback callback, IntPtr state);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr window);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);

    [DllImport("dwmapi.dll", EntryPoint = "DwmGetWindowAttribute")]
    private static extern int GetDwmWindowRect(
        IntPtr window,
        int attribute,
        out NativeRect rect,
        int size
    );

    [DllImport("dwmapi.dll", EntryPoint = "DwmGetWindowAttribute")]
    private static extern int GetDwmWindowInt(
        IntPtr window,
        int attribute,
        out int value,
        int size
    );

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool GetPhysicalCursorPos(out NativePoint point);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool SetPhysicalCursorPos(int x, int y);

    [DllImport("user32.dll")]
    private static extern IntPtr WindowFromPhysicalPoint(NativePoint point);

    [DllImport("user32.dll")]
    private static extern IntPtr GetAncestor(IntPtr window, uint flags);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern uint SendInput(
        uint inputCount,
        [In] NativeInput[] inputs,
        int inputSize
    );

    private static bool TryGetPhysicalRect(IntPtr window, out NativeRect rect)
    {
        return GetDwmWindowRect(
                window,
                DwmwaExtendedFrameBounds,
                out rect,
                Marshal.SizeOf(typeof(NativeRect))) == 0
            && rect.Right > rect.Left
            && rect.Bottom > rect.Top;
    }

    public static MetraWindowInfo[] GetVisibleWindows(int expectedProcessId)
    {
        List<MetraWindowInfo> windows = new List<MetraWindowInfo>();
        EnumWindows(delegate(IntPtr window, IntPtr state)
        {
            uint processId;
            GetWindowThreadProcessId(window, out processId);
            if (processId != (uint)expectedProcessId || !IsWindowVisible(window))
            {
                return true;
            }

            int cloaked;
            if (GetDwmWindowInt(
                    window,
                    DwmwaCloaked,
                    out cloaked,
                    Marshal.SizeOf(typeof(int))) == 0
                && cloaked != 0)
            {
                return true;
            }

            NativeRect rect;
            if (!TryGetPhysicalRect(window, out rect))
            {
                return true;
            }

            windows.Add(new MetraWindowInfo
            {
                Handle = window,
                Left = rect.Left,
                Top = rect.Top,
                Right = rect.Right,
                Bottom = rect.Bottom
            });
            return true;
        }, IntPtr.Zero);
        return windows.ToArray();
    }

    private static void SendMouse(uint flags)
    {
        NativeInput input = new NativeInput();
        input.Type = InputMouse;
        input.Data.Mouse.Flags = flags;
        NativeInput[] inputs = new NativeInput[] { input };
        if (SendInput(1, inputs, Marshal.SizeOf(typeof(NativeInput))) == 1)
        {
            return;
        }

        int error = Marshal.GetLastWin32Error();
        if (error != 0)
        {
            throw new Win32Exception(error, "SendInput failed");
        }
        throw new InvalidOperationException("SendInput was blocked before reaching the desktop");
    }

    public static void ClickCenter(IntPtr window, int holdMilliseconds)
    {
        NativeRect rect;
        if (!IsWindowVisible(window) || !TryGetPhysicalRect(window, out rect))
        {
            throw new InvalidOperationException("The bubble window is no longer visible");
        }

        int centerX = rect.Left + ((rect.Right - rect.Left) / 2);
        int centerY = rect.Top + ((rect.Bottom - rect.Top) / 2);
        NativePoint previousCursor;
        bool restoreCursor = GetPhysicalCursorPos(out previousCursor);
        bool pressed = false;
        bool released = false;
        try
        {
            if (!SetPhysicalCursorPos(centerX, centerY))
            {
                throw new Win32Exception(Marshal.GetLastWin32Error(), "Unable to position the cursor over the bubble");
            }

            NativePoint targetPoint = new NativePoint { X = centerX, Y = centerY };
            IntPtr hitWindow = GetAncestor(WindowFromPhysicalPoint(targetPoint), GaRoot);
            if (hitWindow != window)
            {
                throw new InvalidOperationException("The bubble center is covered by another window");
            }

            SendMouse(MouseEventLeftDown);
            pressed = true;
            Thread.Sleep(Math.Max(1, holdMilliseconds));
            SendMouse(MouseEventLeftUp);
            released = true;
            // Give the native move loop time to consume the release before moving the
            // cursor back, otherwise cursor restoration can look like a real drag.
            Thread.Sleep(100);
        }
        finally
        {
            if (pressed && !released)
            {
                try { SendMouse(MouseEventLeftUp); }
                catch { }
            }
            if (restoreCursor)
            {
                SetPhysicalCursorPos(previousCursor.X, previousCursor.Y);
            }
        }
    }
}
"@

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

function Get-VisibleMetraWindows([int]$ProcessId) {
  return @([MetraWindowSmokeNative]::GetVisibleWindows($ProcessId))
}

function Test-ProcessElevated {
  $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
  try {
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
  } finally {
    $identity.Dispose()
  }
}

function ConvertTo-DiagnosticText($Value) {
  if ($null -eq $Value -or [string]::IsNullOrWhiteSpace([string]$Value)) { return "none" }
  return (([string]$Value -replace "\s+", " ").Trim())
}

function Get-VisibleWindowSummary([int]$ProcessId) {
  $summary = @(Get-VisibleMetraWindows $ProcessId | ForEach-Object {
    "0x$($_.Handle.ToInt64().ToString('X')):$($_.Width)x$($_.Height)@$($_.Left),$($_.Top)"
  }) -join ", "
  if (-not $summary) { return "none" }
  return $summary
}

$resolvedExecutable = (Resolve-Path -LiteralPath $Executable).Path
$port = Get-FreeTcpPort
$hadPreviousArguments = Test-Path Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$previousArguments = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$browserArguments = "--remote-debugging-port=$port --remote-allow-origins=*"
$isElevated = Test-ProcessElevated
$environmentOverrideApplied = $false
$policyOverrideAttempted = $false
$policyParentPath = "SOFTWARE\Policies\Microsoft\Edge\WebView2"
$policyPath = "SOFTWARE\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments"
$policyValueName = [IO.Path]::GetFileName($resolvedExecutable)
$policyKeyExisted = $false
$policyValueExisted = $false
$previousPolicyValue = $null
$previousPolicyValueKind = $null
if ($isElevated) {
  $policyKey = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey($policyPath, $false)
  if ($null -ne $policyKey) {
    $policyKeyExisted = $true
    try {
      if ($policyKey.GetValueNames() -contains $policyValueName) {
        $policyValueExisted = $true
        $previousPolicyValue = $policyKey.GetValue(
          $policyValueName,
          $null,
          [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames
        )
        $previousPolicyValueKind = $policyKey.GetValueKind($policyValueName)
      }
    } finally {
      $policyKey.Dispose()
    }
  }
}
$process = $null
try {
  if ($isElevated) {
    $policyKey = [Microsoft.Win32.Registry]::LocalMachine.CreateSubKey($policyPath)
    if ($null -eq $policyKey) { throw "Unable to open the WebView2 machine policy key" }
    try {
      $policyOverrideAttempted = $true
      $policyKey.SetValue(
        $policyValueName,
        $browserArguments,
        [Microsoft.Win32.RegistryValueKind]::String
      )
    } finally {
      $policyKey.Dispose()
    }
  } else {
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $browserArguments
    $environmentOverrideApplied = $true
  }
  $process = Start-Process -FilePath $resolvedExecutable -PassThru
  $startupTimer = [Diagnostics.Stopwatch]::StartNew()
  $target = $null
  $lastHttpError = $null
  $seenPageUrls = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
  do {
    Start-Sleep -Milliseconds 100
    try {
      $pages = Invoke-RestMethod -Uri "http://127.0.0.1:$port/json" -TimeoutSec 1
      $lastHttpError = $null
      foreach ($page in $pages) {
        $pageUrl = [string]$page.url
        if ($pageUrl) { [void]$seenPageUrls.Add($pageUrl) }
        if ($pageUrl -match "view=bubble") { $target = $page; break }
      }
    } catch {
      $lastHttpError = $_.Exception.Message
    }
    $process.Refresh()
  } while (-not $target -and -not $process.HasExited -and $startupTimer.ElapsedMilliseconds -lt $TimeoutMs)
  if (-not $target) {
    $process.Refresh()
    $processExited = $process.HasExited
    $exitCode = if ($processExited) { [string]$process.ExitCode } else { "n/a" }
    $pageSummary = if ($seenPageUrls.Count -gt 0) {
      @($seenPageUrls | Sort-Object) -join ", "
    } else {
      "none"
    }
    throw (
      "Bubble WebView was not ready for pid={0} process_exited={1} exit_code={2} cdp_override={3} cdp_port={4} last_http_error={5} page_urls={6} visible_windows={7}" -f
        $process.Id,
        $processExited,
        $exitCode,
        $(if ($isElevated) { "HKLM" } else { "environment" }),
        $port,
        (ConvertTo-DiagnosticText $lastHttpError),
        (ConvertTo-DiagnosticText $pageSummary),
        (Get-VisibleWindowSummary $process.Id)
    )
  }

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

  $process.Refresh()
  if ($process.HasExited) { throw "Metra exited before the bubble click test: pid=$($process.Id)" }
  $bubbleWindow = Get-VisibleMetraWindows $process.Id |
    Where-Object {
      $_.Width -ge 24 -and $_.Height -ge 24 -and
      $_.Width -le (2 * $_.Height) -and $_.Height -le (2 * $_.Width)
    } |
    Sort-Object @{ Expression = { [long]$_.Width * [long]$_.Height } },
      @{ Expression = { [Math]::Abs($_.Width - $_.Height) } } |
    Select-Object -First 1
  if (-not $bubbleWindow) {
    throw "No visible bubble-sized top-level window was found for pid=$($process.Id)"
  }

  [MetraWindowSmokeNative]::ClickCenter($bubbleWindow.Handle, 120)

  $minimumPanelWidth = [Math]::Max([int]200, [int]($bubbleWindow.Width * 3))
  $minimumPanelHeight = [Math]::Max([int]260, [int]($bubbleWindow.Height * 3))
  $panelTimer = [Diagnostics.Stopwatch]::StartNew()
  $panelWindow = $null
  do {
    Start-Sleep -Milliseconds 50
    $process.Refresh()
    if ($process.HasExited) { break }
    $panelWindow = Get-VisibleMetraWindows $process.Id |
      Where-Object {
        $_.Handle -ne $bubbleWindow.Handle -and
        $_.Width -ge $minimumPanelWidth -and $_.Height -ge $minimumPanelHeight
      } |
      Sort-Object @{ Expression = { [long]$_.Width * [long]$_.Height }; Descending = $true } |
      Select-Object -First 1
  } while (-not $panelWindow -and $panelTimer.ElapsedMilliseconds -lt $TimeoutMs)
  $panelTimer.Stop()

  if (-not $panelWindow) {
    if ($process.HasExited) {
      throw "Metra exited before opening the panel: pid=$($process.Id)"
    }
    $visibleSummary = @(Get-VisibleMetraWindows $process.Id | ForEach-Object {
      "0x$($_.Handle.ToInt64().ToString('X')):$($_.Width)x$($_.Height)"
    }) -join ", "
    if (-not $visibleSummary) { $visibleSummary = "none" }
    throw "The panel did not become visible after a real bubble click: pid=$($process.Id) visible_windows=$visibleSummary"
  }

  Start-Sleep -Milliseconds 400
  $process.Refresh()
  if ($process.HasExited) {
    throw "Metra exited while confirming that the clicked panel remained visible: pid=$($process.Id)"
  }
  $stablePanelWindow = Get-VisibleMetraWindows $process.Id |
    Where-Object {
      $_.Handle -eq $panelWindow.Handle -and
      $_.Width -ge $minimumPanelWidth -and $_.Height -ge $minimumPanelHeight
    } |
    Select-Object -First 1
  if (-not $stablePanelWindow) {
    throw "The panel became hidden again after the bubble click: pid=$($process.Id) panel_hwnd=0x$($panelWindow.Handle.ToInt64().ToString('X'))"
  }
  $panelWindow = $stablePanelWindow

  Write-Output (
    "rendered-bubble pid={0} changed_pixels={1} bubble_hwnd=0x{2} bubble_size={3}x{4} panel_hwnd=0x{5} panel_size={6}x{7}" -f
      $process.Id,
      $changed,
      $bubbleWindow.Handle.ToInt64().ToString("X"),
      $bubbleWindow.Width,
      $bubbleWindow.Height,
      $panelWindow.Handle.ToInt64().ToString("X"),
      $panelWindow.Width,
      $panelWindow.Height
  )
} finally {
  if ($process) {
    Stop-Process -InputObject $process -Force -ErrorAction SilentlyContinue
    Wait-Process -InputObject $process -Timeout 2 -ErrorAction SilentlyContinue
  }
  if ($policyOverrideAttempted) {
    $policyKey = if ($policyValueExisted) {
      [Microsoft.Win32.Registry]::LocalMachine.CreateSubKey($policyPath)
    } else {
      [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey($policyPath, $true)
    }
    if ($null -ne $policyKey) {
      try {
        if ($policyValueExisted) {
          $policyKey.SetValue($policyValueName, $previousPolicyValue, $previousPolicyValueKind)
        } else {
          $policyKey.DeleteValue($policyValueName, $false)
        }
      } finally {
        $policyKey.Dispose()
      }
    }

    if (-not $policyKeyExisted) {
      $policyKey = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey($policyPath, $false)
      $deletePolicyKey = $false
      if ($null -ne $policyKey) {
        try {
          $deletePolicyKey = (
            $policyKey.GetValueNames().Count -eq 0 -and
            $policyKey.GetSubKeyNames().Count -eq 0
          )
        } finally {
          $policyKey.Dispose()
        }
      }
      if ($deletePolicyKey) {
        $policyParentKey = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey(
          $policyParentPath,
          $true
        )
        if ($null -ne $policyParentKey) {
          try {
            $policyParentKey.DeleteSubKey("AdditionalBrowserArguments", $false)
          } finally {
            $policyParentKey.Dispose()
          }
        }
      }
    }
  }
  if ($environmentOverrideApplied) {
    if ($hadPreviousArguments) {
      $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $previousArguments
    } else {
      Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
    }
  }
}
