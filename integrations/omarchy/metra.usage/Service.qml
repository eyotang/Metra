import QtQuick
import Quickshell
import Quickshell.Io

QtObject {
  id: root
  property var payload: null
  property bool connected: false
  property bool refreshing: true
  property string error: ""
  property double lastRequest: 0
  // The installer puts the compiled collector beside the widget. Omarchy's service loader
  // owns one instance across all monitors, preserving the service's last-good snapshots.
  readonly property string executable: decodeURIComponent(Qt.resolvedUrl("metra").toString().replace(/^file:\/\//, ""))

  function refresh() {
    if (!collector.running || Date.now() - lastRequest < 10000) return
    lastRequest = Date.now()
    refreshing = true
    collector.write("refresh\n")
  }

  property Process collector: Process {
    command: [root.executable, "--watch-json"]
    stdinEnabled: true
    running: true
    stdout: SplitParser {
      onRead: function(line) {
        try {
          var value = JSON.parse(line)
          if (!value.snapshot || !value.settings) throw new Error("invalid payload")
          root.payload = value
          root.connected = true
          root.refreshing = value.snapshot.refreshing === true
          root.error = ""
        } catch (_) {
          root.error = "用量数据格式异常"
        }
      }
    }
    onExited: function(code) {
      root.connected = false
      root.refreshing = false
      root.error = "采集服务已断开，正在重连"
      retry.restart()
    }
  }

  property Timer retry: Timer {
    interval: 10000
    onTriggered: root.collector.running = true
  }
}
