// Optional native smoke test: requires the local Omarchy Quickshell runtime.
// A hidden test shell uses fixture data; no account or user config is changed.
import { cpSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawn } from "node:child_process";

const temporary = mkdtempSync(join(tmpdir(), "metra-qml-test-"));
try {
  for (const directory of ["Commons", "Ui"]) {
    symlinkSync(`/usr/share/omarchy/shell/${directory}`, join(temporary, directory));
  }
  cpSync(resolve(import.meta.dirname, "../integrations/omarchy/metra.usage"), join(temporary, "widget"), { recursive: true });
  writeFileSync(join(temporary, "widget/qmldir"), "Widget 1.0 Widget.qml\nFixtureService 1.0 Service.qml\n");
  const provider = id => ({ provider: id, status: "available", plan: "Test", capturedAt: new Date().toISOString(), quotas: [{ label: "5h", remainingPercent: 50 }] });
  const payload = { snapshot: Object.fromEntries(["cursor", "codex", "claude"].map(id => [id, provider(id)])), settings: {} };
  writeFileSync(join(temporary, "widget/Service.qml"), `import QtQuick
QtObject {
  property var payload: ${JSON.stringify(payload)}
  property bool connected: true
  property bool refreshing: false
  property string error: ""
  function refresh() {}
}`);
  writeFileSync(join(temporary, "shell.qml"), `import QtQuick
import Quickshell
import "widget"
ShellRoot {
  FixtureService { id: fixtureBackend }
  PanelWindow {
    visible: false
    implicitWidth: 500
    implicitHeight: 30
    Widget {
      id: widget
      bar: fakeBar
      settings: ({cycleIntervalMs: 2000})
    }
  }
  QtObject {
    id: fakeBar
    property QtObject shell: QtObject { function serviceFor(id) { return fixtureBackend } }
    property bool vertical: false
    property int barSize: 30
    property string position: "top"
    property string fontFamily: "monospace"
    property color barForeground: "white"
    property color foreground: "white"
    property color urgent: "red"
    property bool foregroundAnimationEnabled: false
    property var activePopout: null
    function hideTooltip(target) {}
    function showTooltip(target, text) {}
    function registerClickTarget(target) {}
    function unregisterClickTarget(target) {}
    function requestPopout(owner) { activePopout = owner }
    function releasePopout(owner) { activePopout = null }
  }
  property int phase: 0
  property real originalWidth: 0
  Timer {
    interval: 2400
    repeat: true
    running: true
    onTriggered: {
      if (phase === 0) {
        if (widget.providerIndex !== 1) { console.error("METRA_UI_FAIL: carousel did not advance"); return }
        originalWidth = widget.implicitWidth
        widget.open()
      } else if (phase === 1) {
        if (widget.providerIndex !== 1) { console.error("METRA_UI_FAIL: carousel moved while panel was open"); return }
        widget.advance(-1)
        if (widget.providerIndex !== 0) { console.error("METRA_UI_FAIL: reverse navigation"); return }
        widget.advance(-1)
        if (widget.providerIndex !== 2) { console.error("METRA_UI_FAIL: navigation wraparound"); return }
        if (widget.implicitWidth !== originalWidth) { console.error("METRA_UI_FAIL: bar width changed"); return }
        widget.close()
        widget.settings = ({cycleIntervalMs: 2000})
      } else {
        if (widget.providerIndex !== 0) { console.error("METRA_UI_FAIL: carousel did not resume"); return }
        console.log("METRA_UI_PASS")
        stop()
      }
      phase++
    }
  }
}`);
  const child = spawn("qs", ["--no-color", "-p", temporary], { stdio: ["ignore", "pipe", "pipe"] });
  let logs = "";
  await new Promise((resolveTest, reject) => {
    const timeout = setTimeout(() => { child.kill(); reject(new Error("Native QML test timed out:\n" + logs)); }, 15000);
    function read(chunk) {
      logs += chunk;
      if (logs.includes("METRA_UI_FAIL") || /TypeError:|ReferenceError:|Unable to load configuration/.test(logs)) {
        clearTimeout(timeout); child.kill(); reject(new Error(logs));
      } else if (logs.includes("METRA_UI_PASS")) {
        clearTimeout(timeout); child.kill(); resolveTest();
      }
    }
    child.stdout.on("data", read);
    child.stderr.on("data", read);
    child.on("error", error => { clearTimeout(timeout); reject(error); });
    child.on("exit", code => {
      if (!logs.includes("METRA_UI_PASS")) { clearTimeout(timeout); reject(new Error(`Quickshell exited (${code}):\n${logs}`)); }
    });
  });
  console.log("Native QML carousel, reverse/wrap navigation, hover-panel pause, resume and fixed width: PASS");
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
