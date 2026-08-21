$ErrorActionPreference = "Stop"

$main = Get-Content -Raw -Encoding UTF8 "$PSScriptRoot\..\src\main.ts"
$i18n = Get-Content -Raw -Encoding UTF8 "$PSScriptRoot\..\src\i18n.ts"
$css = Get-Content -Raw -Encoding UTF8 "$PSScriptRoot\..\src\styles.css"
$types = Get-Content -Raw -Encoding UTF8 "$PSScriptRoot\..\src\types.ts"
$config = Get-Content -Raw -Encoding UTF8 "$PSScriptRoot\..\src-tauri\tauri.conf.json"
$appRust = Get-Content -Raw -Encoding UTF8 "$PSScriptRoot\..\src-tauri\src\app.rs"
$settingsRust = Get-Content -Raw -Encoding UTF8 "$PSScriptRoot\..\src-tauri\src\settings.rs"
$cargo = Get-Content -Raw -Encoding UTF8 "$PSScriptRoot\..\src-tauri\Cargo.toml"
$infoPlist = Get-Content -Raw -Encoding UTF8 "$PSScriptRoot\..\src-tauri\Info.plist"
$waterLogo = Get-Content -Raw -Encoding UTF8 "$PSScriptRoot\..\src\metra-water.svg"
$trayTemplate = Join-Path $PSScriptRoot "..\src-tauri\icons\trayTemplate.png"
$trayColor = Join-Path $PSScriptRoot "..\src-tauri\icons\trayColor.png"
$failures = @()

$bubbleRule = [regex]::Match($css, '\.bubble-shell\s*\{[^}]*\}').Value
$panelRule = [regex]::Match($css, '\.panel\s*\{[^}]*\}').Value
if ($bubbleRule -notmatch 'box-shadow:\s*none') { $failures += "bubble outer shadow remains" }
if ($panelRule -notmatch 'box-shadow:\s*none') { $failures += "panel css shadow remains" }
if ($config -match '"label":\s*"panel"[\s\S]*?"shadow":\s*true') { $failures += "panel native shadow remains" }
if ($main -match 'center\.addEventListener\("click"') { $failures += "click is bound below the pointer-capture target" }
if ($main -match 'refresh-dot' -or $css -match '\.refresh-dot') { $failures += "bubble still renders the stray bottom refresh dot" }
if ($main -notmatch 'metra-water\.svg' -or $main -notmatch 'class="logo-mark"' -or $waterLogo -notmatch 'id="vessel"' -or $waterLogo -notmatch 'id="water"') {
  $failures += "the panel logo is not the circular water-level mark"
}
if (-not (Test-Path $trayTemplate) -or $appRust -notmatch 'trayTemplate\.png' -or $appRust -notmatch 'icon_as_template\(true\)' -or $appRust -notmatch 'MenuBuilder::new') {
  $failures += "the native status item or its monochrome template icon is missing"
}
if (-not (Test-Path $trayColor) -or $appRust -notmatch 'target_os = "windows"' -or $appRust -notmatch 'trayColor\.png') {
  $failures += "Windows does not use the colored water-level tray icon"
}
if ($cargo -notmatch 'features\s*=\s*\[[^\]]*"tray-icon"' -or $appRust -notmatch 'ActivationPolicy::Accessory' -or $infoPlist -notmatch '<key>LSUIElement</key>\s*<true\s*/>') {
  $failures += "macOS is not configured as a Dock-free menu bar app"
}
if ($appRust -notmatch 'TRAY_DETAILS_ID' -or $appRust -notmatch 'TRAY_SETTINGS_ID' -or $appRust -notmatch 'TRAY_REFRESH_ID' -or $appRust -notmatch 'TRAY_TOGGLE_BUBBLE_ID' -or $appRust -notmatch 'TRAY_QUIT_ID') {
  $failures += "the status item menu does not expose the required app actions"
}
if ($main -notmatch 'selectstart' -or $main -notmatch 'getSelection\(\)\?\.removeAllRanges\(\)' -or $css -notmatch '-webkit-user-select:\s*none') {
  $failures += "bubble dragging can still leave a white text-selection highlight"
}
if ($main -notmatch 'calculateBubbleDockTarget\(' -or $main -notmatch 'selectBubbleMonitor\(' -or $main -match 'function clampAndSave\(') {
  $failures += "bubble dragging does not settle against the selected monitor edge"
}
if ($main -match '\bconfirm\(') { $failures += "native confirm can be clipped inside the panel" }
if ($main -match '(?m)^\s*\+\s*$') { $failures += "detail template contains a stray patch marker" }
if (($main -notmatch 'cursorIncludedLimit') -or ($main -notmatch 'CURSOR_PRO_INCLUDED_FALLBACK_CENTS = 2_000') -or ($main -notmatch 'CURSOR_ON_DEMAND_FALLBACK_CENTS = 50_000')) {
  $failures += "legacy Cursor plans do not keep their Included and On-Demand monetary limits"
}
if (($types -notmatch 'QuotaKind\s*=\s*"cursor_models"\s*\|\s*"other_models"\s*\|\s*"grok_bot"') -or ($types -notmatch 'onDemandEnabled\?:\s*boolean') -or ($main -notmatch 'function cursorUltraBlocks\(') -or ($main -notmatch 'function cursorUltraOnDemandBlock\(') -or ($main -match 'CURSOR_ULTRA_INCLUDED_FALLBACK_CENTS')) {
  $failures += "Cursor Ultra does not expose its three quota pools and independent On-Demand state"
}
if ($main -notmatch 'cursor-consent' -or $main -notmatch 'consent-confirm' -or $main -notmatch 'consent-cancel') {
  $failures += "Cursor consent is not rendered inside the application"
}
if ($main -notmatch 'function renderLoadingPanel\(') { $failures += "panel has no immediate loading surface" }
if ($main -notmatch 'const ACTION_TIMEOUT_MS' -or $main -notmatch 'const REFRESH_TIMEOUT_MS') {
  $failures += "user actions do not define explicit timeout budgets"
}
if ($main -notmatch 'function withTimeout<' -or $main -notmatch 'function invokeWithTimeout<') {
  $failures += "user actions do not share a timeout wrapper"
}
if ($main -notmatch 'function showToast\(') { $failures += "user actions have no friendly toast feedback" }
if ($main -notmatch 'function refreshWithFeedback\(' -or $main -notmatch 'usage-updated') {
  $failures += "refresh does not wait for actual usage completion feedback"
}
$refreshFeedback = [regex]::Match($main, 'async function refreshWithFeedback[\s\S]*?\n\}').Value
if ($refreshFeedback -notmatch 'hasInlineProgress = view === "panel" && panelMode === "details"' -or $refreshFeedback -notmatch 'if \(hasInlineProgress\)[\s\S]*?\.action-toast[\s\S]*?else[\s\S]*?showToast') {
  $failures += "details refresh still duplicates inline progress with a loading toast"
}
if ($main -notmatch '"refresh_now", \{ includeCursor \}' -or $main -notmatch 'action === "rescan" \? true : undefined') {
  $failures += "manual refresh cannot skip disabled Cursor while rescan still forces detection"
}
if ($appRust -notmatch 'include_cursor\.unwrap_or_else' -or $appRust -notmatch 'service\s*\.cursor_compat') {
  $failures += "native refresh does not default Cursor selection to compatibility mode"
}
$directInvokes = [regex]::Matches($main, '\binvoke(?:<[^>]+>)?\(').Count
if ($directInvokes -ne 1) { $failures += "some IPC operations bypass the timeout wrapper" }
if ($main -notmatch 'const MENU_PANEL_HEIGHT = 432' -or $appRust -notmatch '"menu" => \(252\.0, 432\.0, PANEL_MODE_MENU\)') { $failures += "menu panel height is not fitted to its content" }
if ($main -notmatch 'const PANEL_GAP = 3') { $failures += "panel gap is not the compact 3px contract" }
if ($appRust -notmatch '\(56\.0 \* scale\)\.round\(\) as u32') { $failures += "native panel position does not scale the fixed bubble window size" }
if ($main -match 'position\.x \+ 64') { $failures += "panel still uses the old hard-coded bubble offset" }
if ($main -match 'HOVER_HIDE_DELAY_MS|panel-hover-changed|showPanel\([\s\S]{0,80}(pointerenter|pointerleave|mouseenter|mouseleave)') {
  $failures += "hover-only panel logic still conflicts with click and context menu"
}
if ($config -notmatch '"minWidth":\s*32' -or $config -notmatch '"maxWidth":\s*56' -or $main -notmatch 'const BUBBLE_IDLE_DELAY_MS = 3_000' -or $main -notmatch 'function idleBubbleValue\(') {
  $failures += "three-second native half-hide with usage-only content is missing"
}
if ($css -notmatch '\.bubble-idle-value[^}]*var\(--provider-color\)' -or $css -notmatch 'data-state="idle"' -or $main -notmatch 'calculateBubblePeekFrame\(') {
  $failures += "idle bubble does not expose provider-colored usage in the resized edge slice"
}
if ($main -notmatch 'shell\.addEventListener\("click"[\s\S]*?showPanel\("details", true\)') {
  $failures += "clicking the bubble does not toggle details"
}
if ($main -notmatch 'function providerLoading\(' -or $main -notmatch 'provider-loading') {
  $failures += "CLI detection has no provider-specific friendly loading state"
}
if ($types -notmatch 'ProviderName\s*=\s*"cursor"\s*\|\s*"codex"\s*\|\s*"claude"' -or $types -notmatch 'claude:\s*ProviderSnapshot' -or $types -notmatch 'claudeBubbleLabel:\s*string' -or $types -notmatch 'bubbleVisibleProviders:\s*ProviderName\[\]') {
  $failures += "frontend wire types do not expose Claude Code as a third provider"
}
if ($types -notmatch 'cursorBubbleColor:\s*string' -or $types -notmatch 'codexBubbleColor:\s*string' -or $types -notmatch 'claudeBubbleColor:\s*string') {
  $failures += "frontend settings do not expose persisted provider colors"
}
if ($main -notmatch 'PROVIDER_ORDER[\s\S]*?"cursor"[\s\S]*?"codex"[\s\S]*?"claude"' -or $main -notmatch 'for \(const provider of PROVIDER_ORDER\)') {
  $failures += "Claude Code does not participate in provider ordering and token gain updates"
}
if ($main -notmatch 'providerCard\("Claude Code",\s*payload\.snapshot\.claude\)' -or $main -notmatch 't\("loading\.usage"\)' -or $i18n -notmatch 'Cursor、Codex 和 Claude Code' -or $i18n -notmatch 'Cursor, Codex, and Claude Code' -or $main -notmatch 'PROVIDER_META\[provider\]\.name' -or $main -notmatch 'setAttribute\("aria-label"') {
  $failures += "details, loading, or accessible copy does not include Claude Code"
}
if ($appRust -notmatch '\(width \* scale\)\.round\(\) as u32' -or $appRust -notmatch '\(height \* scale\)\.round\(\) as u32') {
  $failures += "panel position does not use the requested logical size at the current monitor scale"
}
if ($main -notmatch 'panelRequestSequence' -or $appRust -notmatch 'request_id' -or $appRust -notmatch 'Ordering::Acquire') {
  $failures += "stale left/right click panel requests can still overwrite the latest mode"
}
if ($main -notmatch 'const token = provider\.provider === "codex" \|\| provider\.provider === "claude" \? provider\.tokens : undefined') {
  $failures += "Cursor token metrics are still visible in details"
}
if ($main -notmatch 'function bubblePercent\(' -or $main -notmatch 'Math\.max\(\.\.\.provider\.quotas\.map\(\(quota\) => quota\.usedPercent\)\)' -or $main -notmatch 'Math\.min\(\.\.\.provider\.quotas\.map\(\(quota\) => quota\.remainingPercent\)\)') {
  $failures += "bubble percentage mode does not recalculate used and remaining values"
}
if ($main -notmatch 'data-percent-mode="used"' -or $main -notmatch 'data-percent-mode="remaining"' -or $main -notmatch 'set_bubble_percent_mode') {
  $failures += "bubble percentage mode is missing from the settings menu"
}
if ($types -notmatch 'bubbleSnapEnabled:\s*boolean' -or $settingsRust -notmatch 'bubble_snap_enabled:\s*false' -or $main -notmatch 'data-action="snap"' -or $main -notmatch 'set_bubble_snap_enabled') {
  $failures += "edge snapping is not an opt-in persisted context-menu setting"
}
if ($main -notmatch 'settings-updated' -or $appRust -notmatch 'app\.emit\("settings-updated"') {
  $failures += "bubble percentage changes are not broadcast for immediate rendering"
}
if ($main -notmatch 'handle\.addEventListener\("pointerdown"' -or $main -notmatch 'window\.addEventListener\("pointermove", trackPointerSort, true\)' -or $main -notmatch 'data-bubble-provider' -or $main -notmatch 'set_bubble_display_config') {
  $failures += "bubble provider order cannot be edited by dragging in the details panel"
}
if ($main -notmatch 'finishPointerSort[\s\S]*?updateDragPosition\(lastY\)' -or $main -notmatch 'orderKey\(\) !== initialOrder') {
  $failures += "bubble provider drag does not commit from the final pointer position"
}
if ($main -notmatch 'bubble-config-placeholder' -or $main -notmatch 'translateY\(' -or $css -notmatch '\.bubble-config-item\.dragging' -or $css -notmatch '\.bubble-config-placeholder') {
  $failures += "bubble provider drag has no visible lift and drop feedback"
}
if ($main -notmatch '<button[^>]+class="bubble-config-dot color-trigger \$\{provider\}"' -or $main -notmatch 'aria-haspopup="dialog"' -or $css -notmatch '\.bubble-config-dot::after') {
  $failures += "bubble provider identity repeats initials instead of using color dots"
}
if ($main -notmatch '<span class="provider-dot \$\{provider\.provider\}"' -or $main -notmatch '<span class="provider-dot cursor"' -or $main -match '\$\{name\[0\]\}' -or $css -notmatch '\.provider-dot::after') {
  $failures += "provider cards repeat initials instead of using color dots"
}
if ($main -notmatch 'provider\.stale \? "stale" : provider\.status === "available" \? "available" : "unavailable"' -or $css -notmatch '\.status\.available' -or $css -notmatch '\.status\.unavailable' -or $css -notmatch '\.status\.stale' -or $css -notmatch '\.status\.pending') {
  $failures += "provider availability is not rendered with semantic status badges"
}
if ($main -notmatch 'cursorBubbleLabel' -or $main -notmatch 'codexBubbleLabel' -or $main -notmatch 'claudeBubbleLabel' -or $main -notmatch 'cursorLabel,\s*codexLabel,\s*claudeLabel' -or $main -notmatch 'fallbackLabel:\s*"A"' -or $main -notmatch 'maxlength="3"') {
  $failures += "bubble provider labels are not customizable"
}
if ($main -notmatch 'bubbleRow\([\s\S]*?provider\.provider' -or $main -notmatch 'provider-count-\$\{order\.length\}' -or $css -notmatch '\.bubble-center\.provider-count-3' -or $css -notmatch '\.bubble-row\.healthy strong') {
  $failures += "three provider bubble is not compact or provider colors still depend on row order"
}
if ([regex]::Matches($main, '#[0-9a-f]{6}').Count -lt 55 -or $main -notmatch 'const COLOR_PALETTE' -or $main -notmatch 'COLOR_PALETTE\.flat\(\)') {
  $failures += "the Feishu-style 55-color palette is incomplete"
}
if ($main -notmatch 'className\s*=\s*"color-palette-popover"' -or $main -notmatch 'setAttribute\("role",\s*"dialog"\)' -or $main -notmatch 'aria-pressed=' -or $main -notmatch 'function openColorPalette\(' -or $main -notmatch 'function closeColorPalette\(') {
  $failures += "the provider color trigger has no accessible in-panel palette"
}
if ($main -notmatch 'closeColorPalette\(true\)' -or $main -notmatch 'event\.key === "Escape"' -or $main -notmatch 'pointerdown' -or $main -notmatch 'contains\(event\.target as Node\)') {
  $failures += "the color palette cannot be dismissed before the panel"
}
if ($main -notmatch 'cursorColor,\s*codexColor,\s*claudeColor' -or $appRust -notmatch 'cursor_color:\s*String' -or $settingsRust -notmatch 'BUBBLE_COLOR_PALETTE:\s*\[&str;\s*55\]') {
  $failures += "custom provider colors are not validated and persisted"
}
if ($main -notmatch '--provider-color:' -or $css -notmatch 'var\(--provider-color\)' -or $main -notmatch 'bubbleProviderColor\(') {
  $failures += "selected colors do not synchronize across bubble, config rows, and provider cards"
}
if ($main -notmatch 'provider\.provider === "codex"\s*\|\|\s*provider\.provider === "claude"' -or $main -notmatch 'provider\.provider === "claude"\s*\?\s*t\("provider\.localLifetimeTokens"\)') {
  $failures += "Claude local token metrics are not shown in the details card"
}
if ($appRust -notmatch 'config_dir\.join\("settings\.db"\)' -or $settingsRust -notmatch 'CREATE TABLE IF NOT EXISTS app_settings') {
  $failures += "application settings are not persisted in SQLite"
}
if ($main -notmatch 'function compactTokenDelta\(' -or $main -notmatch '\.toFixed\(2\).*M' -or $main -notmatch '\.toFixed\(2\).*K') {
  $failures += "token gains are not formatted to two decimals with K/M units"
}
if ($main -notmatch 'next - previous' -or $main -notmatch 'TOKEN_GAIN_VISIBLE_MS' -or $css -notmatch '@keyframes token-gain-pop') {
  $failures += "token gain feedback is missing from refresh updates"
}
$nativeSetupStart = $appRust.IndexOf('.setup(|app| {')
$nativeSetupEnd = $appRust.IndexOf('.run(', $nativeSetupStart)
$nativeSetup = if ($nativeSetupStart -ge 0 -and $nativeSetupEnd -gt $nativeSetupStart) { $appRust.Substring($nativeSetupStart, $nativeSetupEnd - $nativeSetupStart) } else { "" }
$nativeBubbleShow = $nativeSetup.IndexOf('let _ = window.show()')
$nativeAutostart = $nativeSetup.IndexOf('let _ = autostart_app.autolaunch()')
if ($nativeBubbleShow -lt 0 -or $nativeAutostart -lt 0 -or $nativeBubbleShow -gt $nativeAutostart) {
  $failures += "native bubble show is blocked behind autostart maintenance"
}
if ($nativeSetup.IndexOf('std::thread::spawn(move || {', $nativeBubbleShow) -lt 0) {
  $failures += "startup maintenance is not delegated to a background thread"
}
$loadPayload = [regex]::Match($main, 'async function loadPayload\(\): Promise<void> \{[\s\S]*?\n\}').Value
if ($loadPayload.IndexOf('if (view === "bubble") renderBubble()') -lt 0 -or $loadPayload.IndexOf('renderBubble()') -gt $loadPayload.IndexOf('invokeWithTimeout<AppPayload>')) {
  $failures += "bubble loading UI is not rendered before data IPC"
}
if ($loadPayload.IndexOf('renderLoadingPanel()') -lt 0 -or $loadPayload.IndexOf('renderLoadingPanel()') -gt $loadPayload.IndexOf('invokeWithTimeout<AppPayload>')) {
  $failures += "panel loading surface is not rendered before data IPC"
}

$showPanel = [regex]::Match($main, 'async function showPanel\([\s\S]*?\n\}').Value
if ($main -notmatch 'const PANEL_SHOW_TIMEOUT_MS = 1_000') {
  $failures += "panel display has no explicit sub-second timeout budget"
}
if ($showPanel -notmatch 'invokeWithTimeout<number>\s*\(\s*"show_panel"') {
  $failures += "panel display is not delegated through one native bridge call"
}
if ($showPanel -match 'WebviewWindow|getByLabel|setSize|emitTo|positionPanel|panel\.show|panel\.setFocus') {
  $failures += "panel display still performs sequential frontend window bridge calls"
}
$showPanelInvokes = [regex]::Matches($showPanel, 'invokeWithTimeout').Count
if ($showPanelInvokes -ne 1) { $failures += "panel display uses more than one frontend IPC roundtrip" }
if ($appRust -notmatch 'fn show_panel\(' -or $appRust -notmatch 'show_panel,') {
  $failures += "native show_panel command is missing or not registered"
}
$nativeShow = [regex]::Match($appRust, 'fn show_panel_window\([\s\S]*?\n\}').Value
if ($nativeShow -notmatch 'is_visible\(\)' -or $nativeShow -notmatch 'should_hide_panel' -or $nativeShow -notmatch 'panel\.hide\(\)') {
  $failures += "native details toggle does not check visibility before hiding"
}
$nativePosition = $nativeShow.IndexOf('set_position')
$nativeShowIndex = $nativeShow.IndexOf('.show()')
if ($nativePosition -lt 0 -or $nativeShowIndex -lt 0 -or $nativePosition -gt $nativeShowIndex) {
  $failures += "native panel command does not position before showing"
}
if ($failures.Count) {
  $failures | ForEach-Object { Write-Error $_ }
  exit 1
}

& (Join-Path $PSScriptRoot "verify-refresh-animation.ps1")

Write-Output "UI contract PASS"
