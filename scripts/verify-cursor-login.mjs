import { readFileSync } from "node:fs";

const main = readFileSync(new URL("../src/main.ts", import.meta.url), "utf8");
const app = readFileSync(new URL("../src-tauri/src/app.rs", import.meta.url), "utf8");
const service = readFileSync(new URL("../src-tauri/src/service.rs", import.meta.url), "utf8");

function requireMatch(source, pattern, message) {
  if (!pattern.test(source)) throw new Error(message);
}

const providerCard = main.match(/function providerCard\([\s\S]*?\n}\n\nfunction bubbleConfigItem/)?.[0] ?? "";
requireMatch(providerCard, /cursorNeedsLogin\s*=\s*provider\.provider\s*===\s*"cursor"\s*&&\s*provider\.status\s*===\s*"not_logged_in"/, "Cursor 未登录必须有独立展示分支");
requireMatch(providerCard, /cursorNeedsLogin\s*\?\s*cursorLoginPrompt\(/, "Cursor 未登录时不能渲染兼容模式额度块");
requireMatch(providerCard, /id="login-cursor"/, "Cursor 未登录卡片必须提供登录按钮");
requireMatch(providerCard, /cursorCanEnableUsage[\s\S]{0,180}provider\.status\s*===\s*"available"[\s\S]{0,180}provider\.status\s*===\s*"desktop_installed"/, "Cursor Agent CLI 或桌面版存在时必须允许显式开启兼容模式");
requireMatch(providerCard, /cursorUsageAvailable\s*=\s*cursorCompat\s*&&\s*provider\.status\s*===\s*"available"/, "兼容额度块只能在实际采集成功后显示");
requireMatch(providerCard, /cursorUsageAvailable\s*\?\s*cursorCostBlocks\(provider\)\s*:\s*quotaRows\(provider\)/, "Cursor 采集失败时必须显示原因，不能显示默认额度");
requireMatch(providerCard, /cursorCanEnableUsage[\s\S]{0,600}id="enable-cursor-usage"/, "Cursor 桌面版状态必须提供显式的精确用量授权按钮");

const loginCursor = main.match(/async function loginCursor\([\s\S]*?\n}\n/)?.[0] ?? "";
requireMatch(loginCursor, /invokeWithTimeout<CursorLoginStart>\("start_cursor_login"/, "登录按钮必须调用原生 Cursor 登录命令");
if (/set_cursor_compat/.test(loginCursor)) throw new Error("登录 Cursor 不得隐式开启精确用量兼容模式");

requireMatch(main, /#login-cursor[\s\S]{0,160}loginCursor\(\)/, "详情面板必须绑定 Cursor 登录按钮");
requireMatch(main, /cursorLoginPending[\s\S]{0,260}recheckCursorLogin\(\)/, "从 Cursor 返回后必须自动重新检测登录状态");
const recheckCursorLogin = main.match(/async function recheckCursorLogin\([\s\S]*?\n}\nfunction enableCursorUsage/)?.[0] ?? "";
requireMatch(recheckCursorLogin, /const updated\s*=\s*await invokeWithTimeout<AppPayload>\("recheck_cursor_login"[\s\S]{0,120}REFRESH_TIMEOUT_MS/, "登录重检必须等待并使用本次原生命令的精确结果");
if (/listen<AppPayload>\("usage-updated"/.test(recheckCursorLogin)) throw new Error("登录重检不得误用无关联的全局刷新事件");
requireMatch(recheckCursorLogin, /cursorLoginPending\s*=\s*false/, "登录重检结束后必须恢复登录按钮，避免无反馈死按钮");
const renderMenu = main.match(/function renderMenu\([\s\S]*?\n}\n\nfunction renderPanel/)?.[0] ?? "";
requireMatch(renderMenu, /payload!\.snapshot\.cursor\.status\s*===\s*"not_logged_in"[\s\S]{0,180}loginCursor\(\)/, "设置菜单也必须在未登录时优先进入 Cursor 登录，而不是弹兼容授权");

requireMatch(app, /const CURSOR_SETTINGS_DEEP_LINK:\s*&str\s*=\s*"cursor:\/\/anysphere\.cursor-deeplink\/settings\/plan-usage"/, "编辑器登录必须使用固定 Cursor 设置深链");
requireMatch(app, /fn start_cursor_login\(/, "缺少 start_cursor_login 原生命令");
requireMatch(app, /fn recheck_cursor_login\(/, "缺少登录后重新检测命令");
requireMatch(app, /generate_handler!\[[\s\S]*start_cursor_login[\s\S]*recheck_cursor_login/, "Cursor 登录命令必须注册到 Tauri IPC");
requireMatch(service, /pub fn refresh_cursor_login_status\(&self\)/, "登录完成后必须仅重新检测 Cursor 状态");

console.log("Cursor login contract: PASS");
