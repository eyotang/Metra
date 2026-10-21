import QtQuick
import QtQuick.Controls
import Quickshell
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "Model.js" as Model

BarWidget {
  id: root
  moduleName: "metra.usage"
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight
  property bool opened: false
  property double nowMs: Date.now()
  // Omarchy owns one service instance for this plugin across all monitors.
  // Unlike a QML singleton, it is recreated when the plugin is hot-reloaded.
  readonly property var backend: bar && bar.shell && bar.shell.serviceFor
    ? (bar.shell.serviceFor(moduleName) || unavailableBackend) : unavailableBackend
  QtObject {
    id: unavailableBackend
    readonly property var payload: null
    readonly property bool connected: false
    readonly property bool refreshing: true
    readonly property string error: "正在连接采集服务"
    function refresh() {}
  }
  readonly property var providers: Model.providers(root.backend.payload)
  property string selectedProviderId: ""
  readonly property int providerIndex: Math.max(0, providers.findIndex(function(p) { return p.provider === root.selectedProviderId }))
  readonly property string headline: Model.headline(providers[providerIndex])
  property string previousHeadline: ""
  property real slideProgress: 0
  property int slideDirection: 1
  readonly property color foreground: Commons.Color.popups.text
  readonly property color dim: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.68)
  readonly property string family: bar ? bar.fontFamily : Style.font.family

  function open() { hideDelay.stop(); opened = true }
  function close() { showDelay.stop(); hideDelay.stop(); opened = false }
  function advance(direction) {
    if (providers.length < 2) return
    previousHeadline = headline
    slideDirection = direction
    selectedProviderId = providers[(providerIndex + direction + providers.length) % providers.length].provider
    slide.restart()
  }
  function hoverChanged() {
    if (button.tooltipHovered || popup.containsMouse) {
      hideDelay.stop()
      if (!opened) showDelay.restart()
    } else {
      showDelay.stop()
      if (opened) hideDelay.restart()
    }
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.headline
    fixedWidth: root.vertical ? -1 : Style.space(76)
    labelVisible: false
    fontSize: Style.font.caption
    active: root.providers.some(function(p) { var n = Model.remaining(p); return n !== null && n <= 10 })
    dimmed: !root.backend.connected
    onTooltipHoveredChanged: root.hoverChanged()
    onWheelMoved: function(delta) { if (delta !== 0) root.advance(delta < 0 ? 1 : -1) }
    onPressed: function(b) {
      if (b === Qt.MiddleButton) root.backend.refresh()
      else root.open()
    }

    Item {
      anchors.fill: parent
      clip: true
      Text {
        width: parent.width
        anchors.verticalCenter: parent.verticalCenter
        x: -root.slideDirection * width * (1 - root.slideProgress)
        visible: slide.running
        text: root.previousHeadline
        color: button.active ? button.activeColor : button.foreground
        font.family: root.family
        font.pixelSize: Style.font.caption
        horizontalAlignment: Text.AlignHCenter
      }
      Text {
        width: parent.width
        anchors.verticalCenter: parent.verticalCenter
        x: root.slideDirection * width * root.slideProgress
        text: root.headline
        color: button.active ? button.activeColor : button.foreground
        font.family: root.family
        font.pixelSize: Style.font.caption
        horizontalAlignment: Text.AlignHCenter
      }
    }
  }

  NumberAnimation {
    id: slide
    target: root
    property: "slideProgress"
    from: 1
    to: 0
    duration: 180
    easing.type: Easing.OutCubic
  }
  Timer {
    id: carousel
    interval: Math.max(2000, Number(root.setting("cycleIntervalMs", 5000)))
    repeat: true
    running: root.providers.length > 1 && !root.opened && !button.tooltipHovered
    onTriggered: root.advance(1)
  }

  Timer {
    id: showDelay
    interval: Math.max(0, Number(root.setting("hoverDelayMs", 180)))
    onTriggered: if (button.tooltipHovered || popup.containsMouse) root.open()
  }
  Timer {
    id: hideDelay
    interval: Math.max(150, Number(root.setting("hideDelayMs", 320)))
    onTriggered: if (!button.tooltipHovered && !popup.containsMouse) root.close()
  }
  Timer {
    interval: 15000
    repeat: true
    running: root.opened
    onTriggered: root.nowMs = Date.now()
  }
  onOpenedChanged: if (opened) nowMs = Date.now()

  PopupCard {
    id: popup
    anchorItem: button
    bar: root.bar
    owner: root
    open: root.opened
    triggerMode: "hover"
    margin: Style.space(6)
    contentWidth: fittedContentWidth(Style.space(350))
    contentHeight: fittedContentHeight(content.implicitHeight)
    onContainsMouseChanged: root.hoverChanged()

    Flickable {
      id: scroller
      anchors.fill: parent
      contentWidth: width
      contentHeight: content.implicitHeight
      clip: true
      boundsBehavior: Flickable.StopAtBounds
      ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

      Column {
        id: content
        width: scroller.width
        spacing: Style.space(14)

        Row {
          width: parent.width
          spacing: Style.space(8)
          Column {
            width: parent.width - refreshButton.width - Style.space(8)
            spacing: Style.space(3)
            Text {
              text: "Metra"
              color: root.foreground
              font.family: root.family
              font.pixelSize: Style.font.title
              font.bold: true
            }
            Text {
              text: root.backend.refreshing ? "正在更新用量…" : "剩余额度 · Cursor / Codex / Claude Code"
              color: root.dim
              font.family: root.family
              font.pixelSize: Style.font.caption
            }
          }
          PanelActionButton {
            id: refreshButton
            iconText: "↻"
            tooltipText: "刷新用量"
            foreground: root.foreground
            enabled: root.backend.connected && !root.backend.refreshing
            onClicked: root.backend.refresh()
          }
        }

        Text {
          width: parent.width
          visible: root.backend.error !== ""
          text: root.backend.error
          textFormat: Text.PlainText
          wrapMode: Text.WordWrap
          color: Commons.Color.urgent
          font.family: root.family
          font.pixelSize: Style.font.caption
        }

        Repeater {
          model: root.providers
          Column {
            id: providerCard
            required property var modelData
            readonly property var provider: modelData
            width: content.width
            spacing: Style.space(7)

            Rectangle { width: parent.width; height: 1; color: root.foreground; opacity: 0.14 }
            Row {
              width: parent.width
              Text {
                width: parent.width * 0.56
                text: Model.name(providerCard.provider.provider)
                color: root.foreground
                font.family: root.family
                font.pixelSize: Style.font.body
                font.bold: true
              }
              Text {
                width: parent.width * 0.44
                text: (providerCard.provider.stale || !root.backend.connected ? "过期 · " : "")
                  + (providerCard.provider.plan || "")
                textFormat: Text.PlainText
                horizontalAlignment: Text.AlignRight
                elide: Text.ElideRight
                color: providerCard.provider.stale || !root.backend.connected ? Commons.Color.urgent : root.dim
                font.family: root.family
                font.pixelSize: Style.font.caption
              }
            }

            Repeater {
              model: providerCard.provider.quotas || []
              Column {
                id: quotaRow
                required property var modelData
                width: providerCard.width
                spacing: Style.space(4)
                Row {
                  width: parent.width
                  Text {
                    width: parent.width * 0.7
                    text: quotaRow.modelData.label
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: root.dim
                    font.family: root.family
                    font.pixelSize: Style.font.caption
                  }
                  Text {
                    width: parent.width * 0.3
                    text: Math.round(quotaRow.modelData.remainingPercent) + "% 剩余"
                    horizontalAlignment: Text.AlignRight
                    color: quotaRow.modelData.remainingPercent <= 10 ? Commons.Color.urgent : root.foreground
                    font.family: root.family
                    font.pixelSize: Style.font.caption
                  }
                }
                Rectangle {
                  width: parent.width
                  height: Style.space(4)
                  radius: height / 2
                  color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
                  Rectangle {
                    width: parent.width * Math.max(0, Math.min(100, quotaRow.modelData.remainingPercent)) / 100
                    height: parent.height
                    radius: parent.radius
                    color: quotaRow.modelData.remainingPercent <= 10 ? Commons.Color.urgent : Commons.Color.accent
                    Behavior on width { NumberAnimation { duration: 160 } }
                  }
                }
                Text {
                  visible: text !== ""
                  text: Model.reset(quotaRow.modelData.resetsAt, root.nowMs)
                  color: root.dim
                  font.family: root.family
                  font.pixelSize: Style.font.caption
                }
              }
            }

            Text {
              width: parent.width
              visible: !(providerCard.provider.quotas || []).length || !!providerCard.provider.stale
              text: Model.status(providerCard.provider)
              textFormat: Text.PlainText
              wrapMode: Text.WordWrap
              color: root.dim
              font.family: root.family
              font.pixelSize: Style.font.caption
            }
            Text {
              width: parent.width
              visible: !!providerCard.provider.tokens
              text: "今日 " + Model.tokens((providerCard.provider.tokens || {}).today)
                + "  ·  累计 " + Model.tokens((providerCard.provider.tokens || {}).lifetime) + " tokens"
              color: root.dim
              font.family: root.family
              font.pixelSize: Style.font.caption
            }
            Text {
              width: parent.width
              visible: text !== ""
              text: Model.costs(providerCard.provider.cost)
              wrapMode: Text.WordWrap
              color: root.dim
              font.family: root.family
              font.pixelSize: Style.font.caption
            }
            Text {
              text: Model.age(providerCard.provider.capturedAt, root.nowMs)
              color: root.dim
              font.family: root.family
              font.pixelSize: Style.font.caption
            }
          }
        }
      }
    }
  }
}
