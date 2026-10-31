# Metra

[English](README.md) · [简体中文](README.zh-CN.md) · **日本語** · [한국어](README.ko.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md)

Metra は **Tauri 2、TypeScript、Rust** で構築された、Cursor・Codex・Claude Code の使用量を表示するネイティブデスクトップアプリです。Windows は WebView2、macOS は WKWebView、Linux は WebKitGTK で描画します。画面端の32pxカプセルを維持し、全プラットフォームで背景ぼかしのない不透明な画面と滑らかな輪郭を使用します。

既定のフローティングバブル、ドラッグ、画面端への吸着、ステータスバー表示、使用量詳細、プロバイダーの表示順・色・ラベル・表示切り替え、日本語を含む7言語の設定に対応します。表示方法は SQLite に保存され、再起動後も維持されます。既存の `dev.metra.desktop/settings.db` と Cursor 互換モードへの明示的な同意を引き継ぎます。

## 開発

最新の安定版 Rust と各 OS のネイティブ開発ツールをインストールしてください。

```sh
npm ci
npm run dev
npm run dev -- -- --settings
npm run frontend:build
cargo test --locked --all-targets
npm run build -- --no-bundle -- --locked
```

`METRA_CONFIG_DIR` でテスト用の設定保存先を指定できます。Linux は X11 / XWayland、WebKitGTK 4.1、GTK 3 / AppIndicator が必要です。

デスクトップ依存のない Omarchy 用コレクターも構築できます。

```sh
cargo build --release --locked --no-default-features --target-dir target/collector
target/collector/release/metra --usage-json
```

開発とパッケージ作成には Node.js 24 以上が必要です。`node scripts/package-native.mjs` はホスト OS 用、macOS の `--universal` は Apple Silicon と Intel 用の DMG を作成します。Windows の NSIS と WebView2 のインストール処理は Tauri が管理します。成果物は `release-assets/` に保存されます。

更新は既存の `latest.json` と署名鍵に対応します。Windows インストール版はユーザーの操作後に署名を検証してインストーラーを起動し、ポータブル版・macOS・Linux は該当リリースのダウンロードページを開きます。詳細な構成・ビルド・公開手順は [English README](README.md)、Omarchy は [専用ガイド](integrations/omarchy/README.md) を参照してください。
