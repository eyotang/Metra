// Presentation only: percentages always come from the existing Rust collectors.
function providers(payload) {
  if (!payload || !payload.snapshot) return []
  var settings = payload.settings || {}
  var order = settings.bubbleProviderOrder || ["cursor", "codex", "claude"]
  var visible = settings.bubbleVisibleProviders || order
  return order.filter(function(id) { return visible.indexOf(id) >= 0 && payload.snapshot[id] })
    .map(function(id) { return payload.snapshot[id] })
}

function name(id) { return ({ cursor: "Cursor", codex: "Codex", claude: "Claude Code" })[id] || id }
function letter(id) { return ({ cursor: "C", codex: "X", claude: "A" })[id] || "?" }

function remaining(provider) {
  var values = (provider.quotas || []).map(function(q) { return q.remainingPercent })
    .filter(function(n) { return typeof n === "number" && isFinite(n) })
  return values.length ? Math.max(0, Math.min(100, Math.min.apply(null, values))) : null
}

function summary(payload) {
  var list = providers(payload)
  if (!list.length) return "Metra · …"
  return list.map(function(p) {
    return headline(p)
  }).join("  ·  ")
}

function headline(provider) {
  if (!provider) return "AI …"
  var value = remaining(provider)
  return letter(provider.provider) + " " + (value === null ? "—" : Math.round(value) + "%")
}

function status(provider) {
  if (provider.message) return provider.message
  return ({ available: "暂无额度数据", desktop_installed: "已安装桌面客户端",
    not_installed: "未安装", not_logged_in: "未登录", unsupported: "暂无额度",
    network_error: "网络暂不可用", protocol_error: "用量读取失败" })[provider.status] || "暂无额度"
}

function tokens(value) {
  if (typeof value !== "number" || !isFinite(value)) return "—"
  if (value >= 1000000) return (value / 1000000).toFixed(1) + "M"
  if (value >= 1000) return (value / 1000).toFixed(1) + "K"
  return String(value)
}

function reset(at, now) {
  if (!at) return ""
  var delta = new Date(at).getTime() - now
  if (!isFinite(delta)) return ""
  if (delta <= 0) return "已到重置时间，等待刷新"
  var minutes = Math.ceil(delta / 60000)
  var days = Math.floor(minutes / 1440)
  var hours = Math.floor(minutes / 60) % 24
  return (days ? days + "天 " : "") + (hours ? hours + "小时 " : "")
    + (minutes % 60) + "分钟后重置"
}

function age(at, now) {
  var minutes = Math.floor((now - new Date(at).getTime()) / 60000)
  if (!isFinite(minutes)) return ""
  if (minutes < 1) return "刚刚更新"
  if (minutes < 60) return minutes + "分钟前更新"
  return Math.floor(minutes / 60) + "小时前更新"
}

function costs(cost) {
  if (!cost) return ""
  var currency = cost.currency === "USD" ? "$" : cost.currency + " "
  function money(cents) { return currency + (cents / 100).toFixed(2) }
  var rows = []
  if (cost.todayUsedCents != null) rows.push("今日费用 " + money(cost.todayUsedCents))
  if (cost.includedUsedCents != null) rows.push("套餐已用 " + money(cost.includedUsedCents)
    + (cost.includedLimitCents != null ? " / " + money(cost.includedLimitCents) : ""))
  if (cost.onDemandUsedCents != null) rows.push("按量已用 " + money(cost.onDemandUsedCents))
  return rows.join(" · ")
}
