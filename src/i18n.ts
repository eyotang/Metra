import type { ProviderName, ProviderStatus } from "./types";
import { ja } from "./locales/ja.ts";
import { ko } from "./locales/ko.ts";

export const SUPPORTED_LOCALES = ["zh-CN", "en", "ja", "ko"] as const;

export type SupportedLocale = (typeof SUPPORTED_LOCALES)[number];
export type LocaleInput = string | readonly string[] | null | undefined;
export type TranslationValues = Readonly<Record<string, string | number | boolean>>;

export const DEFAULT_LOCALE: SupportedLocale = "en";

const zhCN = {
  "common.enabled": "已启用",
  "common.disabled": "未启用",
  "common.loading": "加载中",
  "common.waitingForData": "等待数据",
  "common.retry": "重试",
  "common.cancel": "取消",
  "common.confirm": "确认",
  "common.operation": "操作",
  "status.available": "可用",
  "status.desktopInstalled": "桌面版已安装",
  "status.notInstalled": "未安装",
  "status.notLoggedIn": "未登录",
  "status.unsupported": "暂不可用",
  "status.networkError": "网络错误",
  "status.protocolError": "协议不兼容",
  "status.connecting": "连接中",
  "status.stale": "数据已过期",
  "plan.unknown": "套餐未知",
  "action.timeout": "{action}超时，请稍后重试",
  "action.failed": "{action}失败，请稍后重试",
  "app.usageSubtitle": "额度与用量",
  "app.desktopBubbleSubtitle": "桌面用量气泡",
  "loading.ready": "界面已就绪",
  "loading.usage": "正在读取 Cursor、Codex 和 Claude Code 用量…",
  "refresh.action": "刷新",
  "refresh.inProgress": "正在刷新，请稍候",
  "refresh.reading": "{action}中，正在读取最新用量…",
  "refresh.listen": "建立刷新监听",
  "refresh.start": "启动刷新",
  "refresh.success": "{action}完成，数据已更新",
  "refresh.successCursorSkipped": "{action}完成，Cursor 兼容模式未开启，已跳过",
  "refresh.readUsage": "读取用量",
  "refresh.readUsageFailed": "读取用量失败，请重新打开",
  "refresh.updating": "正在更新",
  "refresh.now": "立即更新",
  "refresh.nowUsage": "立即刷新用量",
  "refresh.refreshing": "正在刷新",
  "refresh.refreshingUsage": "正在刷新用量",
  "refresh.everyMinutes": "每 {minutes} 分钟刷新",
  "refresh.everyMinutesActive": "每 {minutes} 分钟刷新 · 正在刷新…",
  "bubble.loading": "加载中",
  "bubble.snapHint": "拖动后吸附到屏幕边缘",
  "bubble.freeHint": "可拖动到屏幕任意位置",
  "bubble.ariaLabel": "Metra 用量悬浮球，{usage}；{dragHint}，按回车打开详情",
  "bubble.usageSeparator": "，",
  "bubble.error.initializePosition": "初始化悬浮球位置失败",
  "bubble.error.wake": "唤醒悬浮球失败",
  "bubble.error.disableSnap": "关闭自动吸边失败",
  "bubble.error.enableSnap": "开启自动吸边失败",
  "bubble.error.watchMovement": "监听悬浮球移动失败",
  "bubble.error.confirmDragEnd": "无法确认拖动是否结束",
  "bubble.error.startDrag": "无法开始拖动悬浮球",
  "bubble.error.updateFrame": "无法更新悬浮球窗口",
  "bubble.error.snap": "吸附悬浮球失败",
  "bubble.error.hide": "隐藏悬浮球失败",
  "bubble.error.savePosition": "保存气泡位置失败",
  "bubble.action.initializeWindow": "初始化悬浮球窗口",
  "bubble.action.updateFrame": "更新悬浮球窗口",
  "bubble.action.savePosition": "保存气泡位置",
  "quota.noData": "当前数据源未提供额度",
  "quota.remaining": "剩余 {value}%",
  "quota.used": "已用 {value}%",
  "quota.resetsAt": "{date} 重置",
  "quota.moneyRemaining": "剩余 {value}",
  "quota.moneyUsed": "已用 {value}",
  "quota.limit": "限额 {value}",
  "quota.limitWaiting": "限额等待数据",
  "quota.reset": "额度重置",
  "quota.subscription": "订阅额度 · {value}",
  "quota.cursorModelsHint": "包含 Cursor Grok 和 Composer",
  "quota.otherModelsHint": "其他模型额度",
  "quota.grokBotHint": "每周额度",
  "quota.onDemand": "On-Demand",
  "quota.onDemandDisabledHint": "按量付费当前未启用",
  "quota.onDemandUnavailableHint": "按量付费状态暂不可用",
  "provider.connectingTitle": "正在连接 {provider} CLI",
  "provider.connectingDescription": "正在检查安装、登录与用量信息…",
  "provider.localTodayTokens": "本机今日 token",
  "provider.localLifetimeTokens": "本机累计 token",
  "provider.officialLifetimeTokens": "官方累计 token",
  "provider.message.notInstalled": "未检测到 {provider} CLI",
  "provider.message.notLoggedIn": "{provider} 尚未登录",
  "provider.message.networkError": "{provider} 网络暂时不可用，请稍后重试",
  "provider.message.protocolError": "{provider} 返回了无法识别的数据",
  "provider.message.unsupported": "{provider} 当前不可用",
  "cursor.compatRequired": "开启个人兼容模式后可读取精确用量",
  "claude.noLimitLocalTokens": "Claude Code 未提供套餐额度；Token 为本地会话统计",
  "claude.noUsageStats": "Claude Code 当前登录方式未提供额度或 Token 统计",
  "quota.agentRequests": "Agent 请求",
  "quota.cursorTotal": "Cursor 总额度 · {value}",
  "quota.secondary": "{label} · 次级",
  "quota.hours": "{value} 小时",
  "quota.usageLimit": "用量额度",
  "cursor.loginExpired": "Cursor 登录已失效",
  "cursor.loginToContinue": "登录后继续读取额度",
  "cursor.loginExpiredHint": "若 Cursor 仍显示已登录，请先退出后重新登录",
  "cursor.loginAuthorizationHint": "浏览器授权完成后返回 Metra，将自动检测",
  "cursor.loginInCursor": "在 Cursor 中重新登录",
  "cursor.login": "登录 Cursor",
  "cursor.readExactUsage": "读取 Cursor 精确用量",
  "cursor.opening": "正在打开…",
  "cursor.openLogin": "打开 Cursor 登录",
  "cursor.loginAlreadyRunning": "Cursor 登录流程已在进行，请在浏览器中完成授权",
  "cursor.loginOpened": "已打开 Cursor 官方登录，请在浏览器中完成授权",
  "cursor.settingsOpened": "已打开 Cursor 账户设置；若仍显示已登录，请先退出后重新登录",
  "cursor.retryLogin": "重试登录 Cursor",
  "cursor.openLoginFailed": "无法打开 Cursor 登录，请稍后重试",
  "cursor.checkingLogin": "正在检测 Cursor 登录状态…",
  "cursor.checkLogin": "检测 Cursor 登录",
  "cursor.loginSuccess": "Cursor 登录成功，数据已更新",
  "cursor.loginNotDetected": "尚未检测到登录，请在 Cursor 中完成后再返回",
  "cursor.checkLoginFailed": "检测 Cursor 登录失败，请点击刷新重试",
  "cursor.accountUnavailable": "当前无法读取 Cursor 账号或额度",
  "consent.cursorTitle": "开启 Cursor 个人兼容模式？",
  "consent.cursorReadOnly": "Metra 将只读 Cursor 本地状态库中的现有登录令牌，仅向 api2.cursor.sh 和 cursor.com 请求用量。",
  "consent.cursorMemoryOnly": "令牌仅在本次请求的内存中存在，不会保存或写入日志。",
  "consent.enable": "确认开启",
  "consent.enabling": "正在开启…",
  "consent.reading": "正在读取 Cursor 用量，请稍候",
  "consent.enableAction": "开启 Cursor 兼容模式",
  "consent.enabledReading": "兼容模式已开启，正在读取 Cursor 用量",
  "consent.enableFailed": "开启失败：{reason}",
  "consent.enableFailureReason": "请稍后重试",
  "config.title": "悬浮球显示",
  "config.subtitle": "排序 · 显示 · 字符 · 颜色",
  "config.dragOrder": "拖动调整顺序",
  "config.dragProviderOrder": "拖动调整 {provider} 的显示顺序",
  "config.chooseColor": "选择 {provider} 标记颜色",
  "config.chooseCurrentColor": "选择 {provider} 标记颜色，当前 {color}",
  "config.viewStats": "查看 {provider} 统计",
  "config.bubbleLabel": "{provider} 在悬浮球上的显示字符",
  "config.providerVisible": "{provider} 在悬浮球中显示",
  "config.hideProvider": "隐藏 {provider}",
  "config.showProvider": "显示 {provider}",
  "config.keepOne": "悬浮球至少保留一个显示项",
  "config.providerHidden": "已从悬浮球隐藏 {provider}",
  "config.providerShown": "已在悬浮球显示 {provider}",
  "config.save": "保存悬浮球显示设置",
  "config.saveFailed": "保存悬浮球显示设置失败",
  "color.group": "{tone}色",
  "color.swatch": "{tone}{hue}色 {color}",
  "color.hue.red": "红",
  "color.hue.orange": "橙",
  "color.hue.yellow": "黄",
  "color.hue.lime": "青柠",
  "color.hue.green": "绿",
  "color.hue.cyan": "青",
  "color.hue.sky": "天蓝",
  "color.hue.blue": "蓝",
  "color.hue.pink": "粉",
  "color.hue.purple": "紫",
  "color.hue.gray": "灰",
  "color.tone.standard": "标准",
  "color.tone.light": "浅色",
  "color.tone.soft": "柔和",
  "color.tone.deep": "深色",
  "color.tone.dark": "暗色",
  "menu.refreshInterval": "刷新间隔",
  "menu.language": "语言",
  "menu.languageSystem": "自动检测",
  "menu.languageZhCn": "简体中文",
  "menu.languageEnglish": "English",
  "menu.languageJapanese": "日本語",
  "menu.languageKorean": "한국어",
  "menu.switchingLanguage": "正在切换语言…",
  "menu.languageUpdated": "语言已更新",
  "menu.updateLanguageAction": "更新语言",
  "menu.bubblePercentage": "气泡百分比",
  "menu.used": "已用",
  "menu.remaining": "剩余",
  "menu.autoSnap": "自动吸边",
  "menu.autoSnapHint": "松手吸附屏幕边缘，并立即半隐藏",
  "menu.autostart": "开机启动",
  "menu.cursorCompat": "Cursor 个人兼容模式",
  "menu.cursorCompatHint": "只读本地令牌，仅访问 Cursor",
  "menu.rescanCli": "重新检测 CLI",
  "menu.quit": "退出 Metra",
  "menu.switchingPercent": "正在切换为{mode}百分比…",
  "menu.percentSwitched": "气泡已显示{mode}百分比",
  "menu.switchPercentAction": "切换气泡百分比",
  "menu.updatingInterval": "正在更新刷新间隔…",
  "menu.intervalUpdated": "刷新间隔已更新",
  "menu.updateIntervalAction": "更新刷新间隔",
  "menu.rescan": "重新检测",
  "menu.enablingSnap": "正在开启自动吸边…",
  "menu.disablingSnap": "正在关闭自动吸边…",
  "menu.snapEnabled": "自动吸边已开启",
  "menu.snapDisabled": "自动吸边已关闭，可自由放置悬浮球",
  "menu.updateSnapAction": "更新自动吸边",
  "menu.updatingAutostart": "正在更新开机启动…",
  "menu.autostartUpdated": "开机启动设置已更新",
  "menu.updateAutostartAction": "更新开机启动",
  "menu.disablingCompat": "正在关闭兼容模式…",
  "menu.compatDisabled": "Cursor 兼容模式已关闭",
  "menu.disableCompatAction": "关闭 Cursor 兼容模式",
  "menu.quitting": "正在退出…",
  "panel.show": "显示弹窗",
  "panel.openFailed": "打开弹窗失败，请重试",
  "panel.hide": "收起弹窗",
  "panel.syncVisibility": "同步弹窗状态",
  "panel.checkFocus": "检查弹窗焦点",
  "panel.hideFailed": "收起弹窗失败",
} as const;

export type TranslationKey = keyof typeof zhCN;

const en: Record<TranslationKey, string> = {
  "common.enabled": "Enabled",
  "common.disabled": "Disabled",
  "common.loading": "Loading",
  "common.waitingForData": "Waiting for data",
  "common.retry": "Retry",
  "common.cancel": "Cancel",
  "common.confirm": "Confirm",
  "common.operation": "Operation",
  "status.available": "Available",
  "status.desktopInstalled": "Desktop app installed",
  "status.notInstalled": "Not installed",
  "status.notLoggedIn": "Not signed in",
  "status.unsupported": "Unavailable",
  "status.networkError": "Network error",
  "status.protocolError": "Incompatible protocol",
  "status.connecting": "Connecting",
  "status.stale": "Data is out of date",
  "plan.unknown": "Unknown plan",
  "action.timeout": "{action} timed out. Please try again later.",
  "action.failed": "{action} failed. Please try again later.",
  "app.usageSubtitle": "Limits & usage",
  "app.desktopBubbleSubtitle": "Desktop usage bubble",
  "loading.ready": "Interface ready",
  "loading.usage": "Reading Cursor, Codex, and Claude Code usage…",
  "refresh.action": "Refresh",
  "refresh.inProgress": "Refreshing. Please wait.",
  "refresh.reading": "{action} in progress—reading the latest usage…",
  "refresh.listen": "Start refresh listener",
  "refresh.start": "Start refresh",
  "refresh.success": "{action} complete. Data updated.",
  "refresh.successCursorSkipped": "{action} complete. Cursor compatibility mode is off, so Cursor was skipped.",
  "refresh.readUsage": "Read usage",
  "refresh.readUsageFailed": "Could not read usage. Reopen Metra to try again.",
  "refresh.updating": "Updating",
  "refresh.now": "Refresh now",
  "refresh.nowUsage": "Refresh usage now",
  "refresh.refreshing": "Refreshing",
  "refresh.refreshingUsage": "Refreshing usage",
  "refresh.everyMinutes": "Refreshes every {minutes} min",
  "refresh.everyMinutesActive": "Refreshes every {minutes} min · Refreshing…",
  "bubble.loading": "Loading",
  "bubble.snapHint": "Drag to dock at a screen edge",
  "bubble.freeHint": "Drag anywhere on the screen",
  "bubble.ariaLabel": "Metra usage bubble. {usage}. {dragHint}. Press Enter to open details.",
  "bubble.usageSeparator": ", ",
  "bubble.error.initializePosition": "Could not initialize the bubble position",
  "bubble.error.wake": "Could not reveal the bubble",
  "bubble.error.disableSnap": "Could not turn off auto-dock",
  "bubble.error.enableSnap": "Could not turn on auto-dock",
  "bubble.error.watchMovement": "Could not monitor bubble movement",
  "bubble.error.confirmDragEnd": "Could not confirm that dragging ended",
  "bubble.error.startDrag": "Could not start dragging the bubble",
  "bubble.error.updateFrame": "Could not update the bubble window",
  "bubble.error.snap": "Could not dock the bubble",
  "bubble.error.hide": "Could not hide the bubble",
  "bubble.error.savePosition": "Could not save the bubble position",
  "bubble.action.initializeWindow": "Initialize the bubble window",
  "bubble.action.updateFrame": "Update the bubble window",
  "bubble.action.savePosition": "Save bubble position",
  "quota.noData": "This source does not provide limit data",
  "quota.remaining": "{value}% remaining",
  "quota.used": "{value}% used",
  "quota.resetsAt": "Resets {date}",
  "quota.moneyRemaining": "{value} remaining",
  "quota.moneyUsed": "{value} used",
  "quota.limit": "{value} limit",
  "quota.limitWaiting": "Waiting for limit data",
  "quota.reset": "Limit resets",
  "quota.subscription": "Subscription limit · {value}",
  "quota.cursorModelsHint": "Includes Cursor Grok and Composer",
  "quota.otherModelsHint": "Other model limit",
  "quota.grokBotHint": "Weekly limit",
  "quota.onDemand": "On-Demand",
  "quota.onDemandDisabledHint": "On-Demand usage is currently disabled",
  "quota.onDemandUnavailableHint": "On-Demand status is currently unavailable",
  "provider.connectingTitle": "Connecting to the {provider} CLI",
  "provider.connectingDescription": "Checking installation, sign-in, and usage…",
  "provider.localTodayTokens": "Local tokens today",
  "provider.localLifetimeTokens": "Local lifetime tokens",
  "provider.officialLifetimeTokens": "Official lifetime tokens",
  "provider.message.notInstalled": "{provider} CLI was not detected",
  "provider.message.notLoggedIn": "{provider} is not signed in",
  "provider.message.networkError": "{provider} is temporarily unavailable because of a network error. Try again later.",
  "provider.message.protocolError": "{provider} returned data Metra could not recognize",
  "provider.message.unsupported": "{provider} is currently unavailable",
  "cursor.compatRequired": "Enable personal compatibility mode to read exact usage",
  "claude.noLimitLocalTokens": "Claude Code does not provide subscription limits; tokens are counted from local sessions.",
  "claude.noUsageStats": "This Claude Code sign-in method does not provide limits or token totals.",
  "quota.agentRequests": "Agent requests",
  "quota.cursorTotal": "Cursor total limit · {value}",
  "quota.secondary": "{label} · Secondary",
  "quota.hours": "{value} hours",
  "quota.usageLimit": "Usage limit",
  "cursor.loginExpired": "Cursor sign-in expired",
  "cursor.loginToContinue": "Sign in to continue reading limits",
  "cursor.loginExpiredHint": "If Cursor still shows you as signed in, sign out and then sign in again",
  "cursor.loginAuthorizationHint": "Return to Metra after browser authorization; sign-in will be detected automatically",
  "cursor.loginInCursor": "Sign in again in Cursor",
  "cursor.login": "Sign in to Cursor",
  "cursor.readExactUsage": "Read exact Cursor usage",
  "cursor.opening": "Opening…",
  "cursor.openLogin": "Open Cursor sign-in",
  "cursor.loginAlreadyRunning": "Cursor sign-in is already in progress. Complete authorization in your browser.",
  "cursor.loginOpened": "Cursor's official sign-in page is open. Complete authorization in your browser.",
  "cursor.settingsOpened": "Cursor account settings are open. If you still appear signed in, sign out and then sign in again.",
  "cursor.retryLogin": "Retry Cursor sign-in",
  "cursor.openLoginFailed": "Could not open Cursor sign-in. Please try again later.",
  "cursor.checkingLogin": "Checking Cursor sign-in…",
  "cursor.checkLogin": "Check Cursor sign-in",
  "cursor.loginSuccess": "Signed in to Cursor. Data updated.",
  "cursor.loginNotDetected": "Sign-in has not been detected. Finish signing in to Cursor, then return here.",
  "cursor.checkLoginFailed": "Could not check Cursor sign-in. Refresh to try again.",
  "cursor.accountUnavailable": "Cursor account or limit data is currently unavailable",
  "consent.cursorTitle": "Enable Cursor personal compatibility mode?",
  "consent.cursorReadOnly": "Metra will read the existing sign-in token from Cursor's local state database and request usage only from api2.cursor.sh and cursor.com.",
  "consent.cursorMemoryOnly": "The token exists in memory only for this request. It is never saved or written to logs.",
  "consent.enable": "Enable",
  "consent.enabling": "Enabling…",
  "consent.reading": "Reading Cursor usage. Please wait.",
  "consent.enableAction": "Enable Cursor compatibility mode",
  "consent.enabledReading": "Compatibility mode enabled. Reading Cursor usage.",
  "consent.enableFailed": "Could not enable: {reason}",
  "consent.enableFailureReason": "Please try again later.",
  "config.title": "Bubble display",
  "config.subtitle": "Order · Visibility · Label · Color",
  "config.dragOrder": "Drag to reorder",
  "config.dragProviderOrder": "Drag to change {provider}'s display order",
  "config.chooseColor": "Choose {provider} marker color",
  "config.chooseCurrentColor": "Choose {provider} marker color; currently {color}",
  "config.viewStats": "View {provider} statistics",
  "config.bubbleLabel": "{provider} label in the bubble",
  "config.providerVisible": "Show {provider} in the bubble",
  "config.hideProvider": "Hide {provider}",
  "config.showProvider": "Show {provider}",
  "config.keepOne": "Keep at least one item visible in the bubble",
  "config.providerHidden": "{provider} hidden from the bubble",
  "config.providerShown": "{provider} shown in the bubble",
  "config.save": "Save bubble display settings",
  "config.saveFailed": "Could not save bubble display settings",
  "color.group": "{tone} colors",
  "color.swatch": "{tone} {hue} {color}",
  "color.hue.red": "red",
  "color.hue.orange": "orange",
  "color.hue.yellow": "yellow",
  "color.hue.lime": "lime",
  "color.hue.green": "green",
  "color.hue.cyan": "cyan",
  "color.hue.sky": "sky blue",
  "color.hue.blue": "blue",
  "color.hue.pink": "pink",
  "color.hue.purple": "purple",
  "color.hue.gray": "gray",
  "color.tone.standard": "standard",
  "color.tone.light": "light",
  "color.tone.soft": "soft",
  "color.tone.deep": "deep",
  "color.tone.dark": "dark",
  "menu.refreshInterval": "Refresh interval",
  "menu.language": "Language",
  "menu.languageSystem": "Auto detect",
  "menu.languageZhCn": "简体中文",
  "menu.languageEnglish": "English",
  "menu.languageJapanese": "日本語",
  "menu.languageKorean": "한국어",
  "menu.switchingLanguage": "Switching language…",
  "menu.languageUpdated": "Language updated",
  "menu.updateLanguageAction": "Update language",
  "menu.bubblePercentage": "Bubble percentage",
  "menu.used": "Used",
  "menu.remaining": "Remaining",
  "menu.autoSnap": "Auto-dock",
  "menu.autoSnapHint": "Dock and peek immediately on release",
  "menu.autostart": "Launch at startup",
  "menu.cursorCompat": "Cursor compatibility",
  "menu.cursorCompatHint": "Local token · Cursor only",
  "menu.rescanCli": "Rescan CLIs",
  "menu.quit": "Quit Metra",
  "menu.switchingPercent": "Switching to {mode} percentage…",
  "menu.percentSwitched": "Bubble now shows {mode} percentage",
  "menu.switchPercentAction": "Switch bubble percentage",
  "menu.updatingInterval": "Updating refresh interval…",
  "menu.intervalUpdated": "Refresh interval updated",
  "menu.updateIntervalAction": "Update refresh interval",
  "menu.rescan": "Rescan",
  "menu.enablingSnap": "Turning on auto-dock…",
  "menu.disablingSnap": "Turning off auto-dock…",
  "menu.snapEnabled": "Auto-dock enabled",
  "menu.snapDisabled": "Auto-dock disabled. Place the bubble anywhere.",
  "menu.updateSnapAction": "Update auto-dock",
  "menu.updatingAutostart": "Updating launch-at-startup…",
  "menu.autostartUpdated": "Launch-at-startup setting updated",
  "menu.updateAutostartAction": "Update launch-at-startup",
  "menu.disablingCompat": "Turning off compatibility mode…",
  "menu.compatDisabled": "Cursor compatibility mode disabled",
  "menu.disableCompatAction": "Disable Cursor compatibility mode",
  "menu.quitting": "Quitting…",
  "panel.show": "Show panel",
  "panel.openFailed": "Could not open the panel. Please try again.",
  "panel.hide": "Hide panel",
  "panel.syncVisibility": "Sync panel visibility",
  "panel.checkFocus": "Check panel focus",
  "panel.hideFailed": "Could not hide the panel",
};

export type TranslationCatalog = Readonly<Record<SupportedLocale, Readonly<Record<TranslationKey, string>>>>;

export const translations: TranslationCatalog = { "zh-CN": zhCN, en, ja, ko };

function canonicalizeLocale(value: string): string | null {
  const normalized = value.trim().replaceAll("_", "-");
  if (!normalized) return null;
  try {
    return Intl.getCanonicalLocales(normalized)[0] ?? null;
  } catch {
    return null;
  }
}

function matchLocale(value: string): SupportedLocale | null {
  const canonical = canonicalizeLocale(value)?.toLowerCase();
  if (!canonical) return null;
  if (canonical === "zh" || canonical.startsWith("zh-hans") || canonical.startsWith("zh-cn") || canonical.startsWith("zh-sg")) return "zh-CN";
  if (canonical === "en" || canonical.startsWith("en-")) return "en";
  if (canonical === "ja" || canonical.startsWith("ja-")) return "ja";
  if (canonical === "ko" || canonical.startsWith("ko-")) return "ko";
  return null;
}

export function resolveLocale(input: LocaleInput, fallback: SupportedLocale = DEFAULT_LOCALE): SupportedLocale {
  const candidates = typeof input === "string" ? [input] : input ?? [];
  for (const candidate of candidates) {
    const matched = matchLocale(candidate);
    if (matched) return matched;
  }
  return fallback;
}

export function runtimeLocaleCandidates(): string[] {
  const candidates: string[] = [];
  if (typeof navigator !== "undefined") {
    candidates.push(...(navigator.languages ?? []));
    if (navigator.language) candidates.push(navigator.language);
  }
  try {
    candidates.push(Intl.DateTimeFormat().resolvedOptions().locale);
  } catch {
    // A browser without Intl support will use the English fallback.
  }
  return candidates;
}

export function detectLocale(
  candidates: LocaleInput = runtimeLocaleCandidates(),
  fallback: SupportedLocale = DEFAULT_LOCALE,
): SupportedLocale {
  return resolveLocale(candidates, fallback);
}

function interpolate(template: string, values: TranslationValues): string {
  return template.replace(/\{([a-zA-Z][\w]*)\}/g, (placeholder, name: string) =>
    Object.hasOwn(values, name) ? String(values[name]) : placeholder,
  );
}

function translateForLocale(
  locale: SupportedLocale,
  key: TranslationKey,
  values: TranslationValues = {},
): string {
  return interpolate(translations[locale][key], values);
}

const PROVIDER_NAMES: Record<ProviderName, string> = {
  cursor: "Cursor",
  codex: "Codex",
  claude: "Claude Code",
};

export function localizeProviderMessage(
  message: string | null | undefined,
  provider: ProviderName,
  status: ProviderStatus,
  locale: SupportedLocale = i18n.locale,
): string | undefined {
  if (!message) return undefined;
  if (locale === "zh-CN" || !/[\p{Script=Han}]/u.test(message)) return message;
  const exactKey = new Map<string, TranslationKey>([
    ["开启个人兼容模式后可读取精确用量", "cursor.compatRequired"],
    ["Claude Code 未提供套餐额度；Token 为本地会话统计", "claude.noLimitLocalTokens"],
    ["Claude Code 当前登录方式未提供额度或 Token 统计", "claude.noUsageStats"],
  ]).get(message);
  if (exactKey) return translateForLocale(locale, exactKey);
  const key: Partial<Record<ProviderStatus, TranslationKey>> = {
    not_installed: "provider.message.notInstalled",
    not_logged_in: "provider.message.notLoggedIn",
    network_error: "provider.message.networkError",
    protocol_error: "provider.message.protocolError",
    unsupported: "provider.message.unsupported",
  };
  const statusKey = key[status];
  return statusKey ? translateForLocale(locale, statusKey, { provider: PROVIDER_NAMES[provider] }) : undefined;
}

export function localizeQuotaLabel(
  label: string,
  locale: SupportedLocale = i18n.locale,
): string {
  if (locale === "zh-CN" || !/[\p{Script=Han}]/u.test(label)) return label;
  if (label === "Agent 请求") return translateForLocale(locale, "quota.agentRequests");
  const total = label.match(/^Cursor 总额度 · (.+)$/u);
  if (total) return translateForLocale(locale, "quota.cursorTotal", { value: total[1] });
  const secondary = label.match(/^(.+) · 次级$/u);
  if (secondary) {
    return translateForLocale(locale, "quota.secondary", { label: localizeQuotaLabel(secondary[1], locale) });
  }
  const hours = label.match(/^(\d+(?:\.\d+)?)\s*小时$/u);
  if (hours) return translateForLocale(locale, "quota.hours", { value: hours[1] });
  return translateForLocale(locale, "quota.usageLimit");
}

export interface DateFormatOptions extends Intl.DateTimeFormatOptions {
  fallback?: string;
}

export interface NumberFormatOptions extends Intl.NumberFormatOptions {
  fallback?: string;
}

export interface I18n {
  readonly locale: SupportedLocale;
  t(key: TranslationKey, values?: TranslationValues): string;
  setLocale(locale: LocaleInput): SupportedLocale;
  subscribe(listener: (locale: SupportedLocale) => void): () => void;
  formatNumber(value: number | null | undefined, options?: NumberFormatOptions): string;
  formatDate(value: Date | string | number | null | undefined, options?: DateFormatOptions): string;
  formatDateTime(value: Date | string | number | null | undefined, options?: DateFormatOptions): string;
  formatCurrency(value: number | null | undefined, currency?: string, options?: NumberFormatOptions): string;
  formatPercentage(value: number | null | undefined, options?: NumberFormatOptions): string;
}

export interface CreateI18nOptions {
  locale?: LocaleInput;
  fallbackLocale?: SupportedLocale;
  catalog?: TranslationCatalog;
}

export function createI18n(options: CreateI18nOptions = {}): I18n {
  const fallbackLocale = options.fallbackLocale ?? DEFAULT_LOCALE;
  const catalog = options.catalog ?? translations;
  let locale = resolveLocale(options.locale ?? runtimeLocaleCandidates(), fallbackLocale);
  const listeners = new Set<(nextLocale: SupportedLocale) => void>();

  const formatNumber = (value: number | null | undefined, options: NumberFormatOptions = {}): string => {
    const { fallback = "—", ...intlOptions } = options;
    return value === null || value === undefined || !Number.isFinite(value)
      ? fallback
      : new Intl.NumberFormat(locale, intlOptions).format(value);
  };

  const formatDate = (
    value: Date | string | number | null | undefined,
    options: DateFormatOptions = {},
  ): string => {
    const { fallback = "—", ...intlOptions } = options;
    if (value === null || value === undefined || value === "") return fallback;
    const date = value instanceof Date ? value : new Date(value);
    return Number.isNaN(date.getTime()) ? fallback : new Intl.DateTimeFormat(locale, intlOptions).format(date);
  };

  return {
    get locale() {
      return locale;
    },
    t(key, values = {}) {
      const template = catalog[locale][key] ?? catalog[fallbackLocale][key];
      return interpolate(template, values);
    },
    setLocale(input) {
      const nextLocale = resolveLocale(input, fallbackLocale);
      if (nextLocale === locale) return locale;
      locale = nextLocale;
      for (const listener of listeners) listener(locale);
      return locale;
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    formatNumber,
    formatDate,
    formatDateTime(value, options = {}) {
      const hasDateTimeOptions = Object.keys(options).some((key) => key !== "fallback");
      return formatDate(value, hasDateTimeOptions ? options : {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
        fallback: options.fallback,
      });
    },
    formatCurrency(value, currency = "USD", options = {}) {
      return formatNumber(value, { style: "currency", currency, ...options });
    },
    formatPercentage(value, options = {}) {
      return formatNumber(value === null || value === undefined ? value : value / 100, {
        style: "percent",
        maximumFractionDigits: 0,
        ...options,
      });
    },
  };
}

export function applyDocumentLocale(locale: SupportedLocale, root: HTMLElement = document.documentElement): void {
  root.lang = locale;
}

export const i18n = createI18n();
export const t = i18n.t.bind(i18n);
export const formatNumber = i18n.formatNumber.bind(i18n);
export const formatDate = i18n.formatDate.bind(i18n);
export const formatDateTime = i18n.formatDateTime.bind(i18n);
export const formatCurrency = i18n.formatCurrency.bind(i18n);
export const formatPercentage = i18n.formatPercentage.bind(i18n);
