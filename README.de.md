# Metra

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Français](README.fr.md) · **Deutsch** · [Русский](README.ru.md)

Metra überwacht die Nutzung von Cursor, Codex und Claude Code mit **Tauri 2, TypeScript und Rust**. Windows verwendet WebView2, macOS WKWebView und Linux WebKitGTK. Alle Oberflächen sind undurchsichtig, ohne Hintergrundunschärfe und mit glatten Konturen.

Die verschiebbare 56-Pixel-Blase kann am Bildschirmrand zu einer 32-Pixel-Kapsel einklappen. Darüberfahren klappt sie auf; ein Klick oder die Eingabetaste öffnet Details. Im Statusleistenmodus wechseln die sichtbaren Anbieter alle fünf Sekunden. Details zeigen Kontostatus, Limits, Rücksetzzeiten, Tokens und Kosten. Gespeichert werden Anbieterreihenfolge, Sichtbarkeit, Beschriftungen, Farben, Sprache, Aktualisierungsintervall und Autostart. Sieben Sprachen stehen mit automatischer Erkennung bereit: Englisch, vereinfachtes Chinesisch, Japanisch, Koreanisch, Französisch, Deutsch und Russisch.

Bestehende Einstellungen unter `dev.metra.desktop/settings.db` bleiben erhalten. Cursor-Zugangsdaten werden erst nach ausdrücklicher Zustimmung gelesen. Bei fehlgeschlagener Aktualisierung bleiben die letzten Daten als veraltet markiert sichtbar.

## Entwicklung

Benötigt werden stabiles Rust, Node.js 24+ und die Tauri-Voraussetzungen: Xcode-Kommandozeilenwerkzeuge unter macOS; Visual-Studio-C++-Buildtools, Windows SDK und WebView2 unter Windows; WebKitGTK 4.1, GTK 3 und AppIndicator unter Linux (X11/XWayland für positionierte Fenster).

```sh
npm ci
npm run dev
npm run dev -- -- --settings
npm run frontend:build
npm run check
cargo test --locked --all-targets
npx playwright install chromium webkit
npm run test:ui
npm run build -- --no-bundle -- --locked
```

`METRA_CONFIG_DIR` legt ein separates Verzeichnis für Testeinstellungen fest. Das Schließen eines Panels beendet Metra nicht; zum Beenden das Menü verwenden. Siehe [Linux-Einrichtung](docs/linux.md).

Der Omarchy-Kollektor benötigt weder Node noch Desktop-Abhängigkeiten:

```sh
cargo build --release --locked --no-default-features --target-dir target/collector
target/collector/release/metra --usage-json
target/collector/release/metra --watch-json
```

Der Überwachungsmodus akzeptiert `refresh` über die Standardeingabe und endet, wenn diese geschlossen wird. Siehe [Omarchy-Anleitung](integrations/omarchy/README.md).

## Pakete und Updates

```sh
node scripts/package-native.mjs
# Universelles macOS-DMG für Apple Silicon und Intel:
rustup target add aarch64-apple-darwin x86_64-apple-darwin
node scripts/package-native.mjs --universal
```

Die Dateien landen in `release-assets/`: macOS-DMG, Windows-NSIS-Installer pro Benutzer und portable EXE sowie Linux-Debian-Paket. Die portable Windows-Version benötigt WebView2. `CARGO_TARGET_DIR` wird unterstützt.

Für eine lokale macOS-Installation die App bauen und `npm run install:local` ausführen. Das sichert die bisherige App, erhält Einstellungen, entfernt erweiterte Attribute und prüft die Signatur. Eine heruntergeladene App nach Applications verschieben und vor dem ersten Start ausführen:

```sh
xattr -cr /Applications/Metra.app
```

Updates verwenden die vorhandene `latest.json` und den Minisign-Schlüssel. Die installierte Windows-Version prüft nach ausdrücklicher Benutzeraktion die Signatur und startet den Installer. Portable Versionen, macOS und Linux öffnen die Downloadseite zur manuellen Installation.

Architektur und CI-Prüfungen stehen im [englischen README](README.md). [MIT-Lizenz](LICENSE); Tauri verwendet MIT/Apache-2.0, Inter behält seine OFL-Lizenz unter `assets/fonts/`.
