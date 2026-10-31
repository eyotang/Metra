# Metra

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Français](README.fr.md) · **Deutsch** · [Русский](README.ru.md)

## Überblick

**Die Nutzung Ihrer KI-Codingtools auf einen Blick.**

Limits, Tokens und Kosten von Cursor, Codex und Claude Code in einer kleinen Desktop-Anzeige. Für **macOS, Windows und Linux**.

- **Immer im Blick.** Blase frei verschieben, am Rand andocken oder die Statusleiste nutzen.
- **Drei Tools, ein Ort.** Verfügbare Limits, Rücksetzzeiten und Nutzung schnell prüfen.
- **Nach Ihren Wünschen.** Reihenfolge, Beschriftungen, Farben und Intervalle anpassen. Sieben Sprachen mit automatischer Erkennung.

| Nutzungsdetails | Anzeigeeinstellungen |
| :---: | :---: |
| <img src="docs/assets/readme/usage-demo.png" alt="Nutzungsdetails" width="320"> | <img src="docs/assets/readme/settings-demo.png" alt="Anzeigeeinstellungen" width="320"> |

Aktuelle Oberfläche mit simulierten Daten, ohne echte Kontoinformationen. Verfügbare Daten hängen vom Anbieter und der Anmeldung ab.

## Erste Schritte

**[Neueste Version herunterladen](https://github.com/eyotang/Metra/releases/latest)**

| System | Datei | Installation |
| --- | --- | --- |
| macOS | `*-macos-universal.dmg` | DMG öffnen und Metra nach Applications ziehen. |
| Windows x64 | `*-windows-x64-setup.exe` | Installer starten; die portable EXE benötigt WebView2. |
| Linux x64 | `*-linux-x64.deb` | Mit `sudo apt install ./Metra-*-linux-x64.deb` installieren. |

Unter macOS nach dem Verschieben nach Applications vor dem ersten Start ausführen, um das Quarantäneattribut zu entfernen:

```sh
sudo xattr -dr com.apple.quarantine /Applications/Metra.app
```

Metra starten, bei den verwendeten Tools anmelden und auf die Blase klicken. Für genaue Cursor-Daten die **Cursor-Kompatibilität** in den Einstellungen aktivieren und dem Lesen lokaler Zugangsdaten zustimmen. Die Blase lässt sich ziehen und mit automatischem Andocken am Rand einklappen.

Sprache, Aktualisierungsintervall und Autostart in den Einstellungen ändern. Das Schließen eines Panels lässt Metra weiterlaufen; über Einstellungen oder Tray-Menü beenden.

[Linux-Einrichtung](docs/linux.md) · [Omarchy-Widget](integrations/omarchy/README.md)

## Entwickeln und beitragen

**Node.js 24+**, **stabiles Rust** und die folgenden Systemtools installieren. Repository forken, eigenen Fork klonen und im Stammverzeichnis ausführen:

| System | Werkzeuge |
| --- | --- |
| macOS | Xcode-Kommandozeilenwerkzeuge |
| Windows | Visual Studio C++ Buildtools, Windows SDK, WebView2 |
| Linux | WebKitGTK 4.1, GTK 3, AppIndicator; X11/XWayland für schwebende Fenster |

```sh
npm ci
npm run dev
```

Vor dem Einreichen einer PR:

```sh
npm run check
cargo test --locked --all-targets
npx playwright install chromium webkit
npm run test:ui
```

UI: `frontend/`; Datenerfassung: `src/providers/`; Aktualisierung: `src/service.rs`. `METRA_CONFIG_DIR` trennt Testeinstellungen von Ihren eigenen.

Jede PR auf ein Problem beschränken und Problem, Änderung und Prüfung beschreiben. Bei UI-Änderungen Screenshots beifügen; betroffene Übersetzungen und README-Dateien in allen sieben Sprachen aktualisieren.

Lokale macOS-Installation: `npm run build`, anschließend `npm run install:local`.

[Referenz für Build, Pakete und Kollektor](docs/development.md) · [Issues](https://github.com/eyotang/Metra/issues) · [Pull requests](https://github.com/eyotang/Metra/pulls)

[MIT-Lizenz.](LICENSE)
