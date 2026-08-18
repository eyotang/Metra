param(
  [Parameter(Mandatory = $true)][string]$Executable,
  [switch]$AttachExisting,
  [int]$TimeoutMs = 3000
)

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.Windows.Forms
if (-not ("MetraWindowProbe" -as [type])) {
  Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class MetraWindowProbe {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  public delegate bool EnumWindowsProc(IntPtr window, IntPtr state);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindowsProc callback, IntPtr state);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr window);
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr window, out RECT rect);
  public static RECT[] VisibleRects(uint targetProcessId) {
    var result = new List<RECT>();
    EnumWindows((window, state) => {
      uint processId;
      RECT rect;
      GetWindowThreadProcessId(window, out processId);
      if (processId == targetProcessId && IsWindowVisible(window) && GetWindowRect(window, out rect)) result.Add(rect);
      return true;
    }, IntPtr.Zero);
    return result.ToArray();
  }
}
"@
}

$resolvedExecutable = (Resolve-Path -LiteralPath $Executable).Path
$ownsProcess = -not $AttachExisting
if ($AttachExisting) {
  $process = Get-Process | Where-Object { $_.Path -eq $resolvedExecutable } | Select-Object -First 1
  if (-not $process) { throw "Metra process is not running: $resolvedExecutable" }
} else {
  $process = Start-Process -FilePath $resolvedExecutable -PassThru
}

try {
  $deadline = [Environment]::TickCount64 + $TimeoutMs
  $visibleBubble = $null
  do {
    $rects = [MetraWindowProbe]::VisibleRects([uint32]$process.Id)
    foreach ($rect in $rects) {
      $width = $rect.Right - $rect.Left
      $height = $rect.Bottom - $rect.Top
      $bubbleSized = $width -ge 40 -and $width -le 180 -and $height -ge 40 -and $height -le 180
      $onScreen = [Windows.Forms.Screen]::AllScreens | Where-Object {
        $_.WorkingArea.Right -gt $rect.Left -and $_.WorkingArea.Left -lt $rect.Right -and
        $_.WorkingArea.Bottom -gt $rect.Top -and $_.WorkingArea.Top -lt $rect.Bottom
      }
      if ($bubbleSized -and $onScreen) {
        $visibleBubble = "visible-bubble pid=$($process.Id) rect=$($rect.Left),$($rect.Top),$width,$height"
        break
      }
    }
    if ($visibleBubble) { break }
    Start-Sleep -Milliseconds 50
    $process.Refresh()
  } while (-not $process.HasExited -and [Environment]::TickCount64 -lt $deadline)
  if ($visibleBubble) {
    Write-Output $visibleBubble
    return
  }
  throw "No visible on-screen bubble window for pid=$($process.Id)"
} finally {
  if ($ownsProcess -and -not $process.HasExited) {
    Stop-Process -Id $process.Id -Force
    Wait-Process -Id $process.Id -Timeout 2 -ErrorAction SilentlyContinue
  }
}
