# Metra

[English](README.md) · [简体中文](README.zh-CN.md) · **日本語** · [한국어](README.ko.md)

Metra は、Cursor、Codex、Claude Code のログイン状態、使用量上限、リセット時刻、トークン累計、利用可能額を確認できる、軽量なクロスプラットフォーム対応デスクトップバブルです。

## 機能

- Windows と macOS に対応した透明な最前面表示バブルです。自由なドラッグとマルチディスプレイに対応しています。画面端への自動吸着はコンテキストメニューから有効にでき、初期設定では無効です。
- 自動吸着を有効にすると、初めて画面端へドラッグして離した時点で、待機時間なしに 32 px だけ見える半隠れ状態へ直ちに移行します。マウスを重ねる、フォーカスする、またはクリックすると、バブル全体がすぐに表示されます。再表示後に半隠れ状態へ戻る際は、通常どおり待機時間が適用されます。
- ローカルの `cursor-agent` / `agent`、`codex`、`claude` CLI を自動検出します。
- 公式の Codex App Server を使って、複数の上限期間とトークン累計を取得します。
- 必要に応じて Cursor の公式ログインフローを開きます。互換モードを無断で有効にしたり、CLI を自動でインストールしたりすることはありません。個人の正確な使用量を取得するには、別途同意が必要です。Ultra では Cursor Models、Other Models、Grok Bot の週間上限、On-Demand を個別に表示します。Team などの従来プランでは、既存の金額表示を維持します。
- `claude auth status --json` で Claude Code のログイン状態を確認し、利用可能な場合はローカルセッションのトークン数を読み取り専用で集計します。Anthropic API キーでログインしている場合、組織管理者は Admin API キーを明示的に設定し、選択したキーアクターの UTC 当日分の公式トークン使用量と推定コストを取得できます。API キーやカスタムゲートウェイから残りの上限を取得できない場合は、架空の割合を表示せず、上限データがないことを通知します。
- プロバイダーごとに表示・非表示を切り替え、6 点ハンドルで並べ替え、バブル内のラベルとマーカー色を個別に変更できます。内蔵の 55 色パレットは、ほかの設定とともに SQLite に保存されます。
- 左クリックで詳細を開き、パネル内の更新アイコンから使用量を更新できます。コンテキストメニューの先頭には言語セレクターがあり、更新間隔、自動起動、互換モード、再検出、終了も設定できます。更新操作は重複して表示しません。
- 更新に失敗しても直近の正常な結果を保持し、古いデータであることを明確に示します。
- v0.1.39 以降、Windows ポータブル版を含む対応配布形式は、起動後まもなく GitHub Releases にある単一の `latest.json` を確認し、実行中は 24 時間ごとに再確認します。Windows NSIS インストール版は承認後にアプリ内更新し、macOS と Windows ポータブル版は同じマニフェストでバージョンを検出して Release ページを開き、手動でダウンロードします。
- インターフェースは英語、簡体字中国語（`zh-CN`）、日本語、韓国語に対応しています。「自動検出」は OS またはブラウザの言語に従い、手動で選んだ言語はすぐに反映され、再起動後も保持されます。

<p align="center">
  <a href="docs/assets/readme/metra-readme-hero.png">
    <img src="docs/assets/readme/metra-readme-hero-1920.jpg" alt="AI 使用量バブルと、プライバシーに配慮して情報を伏せた Cursor、Codex、Claude Code の使用量パネルを示す Metra のプロモーション画像。" width="100%">
  </a>
</p>

## 開発

Rust stable、Node.js 22.13 以降、npm、および使用するプラットフォーム向けの Tauri システム依存関係が必要です。

```text
npm install
npm run check
npm run verify:i18n
npm test
npm run dev
```

### Windows

Windows の PowerShell では、`package.json` で固定されている pnpm 11.22.0 を使用して、次の順序でポータブル版をビルドしてください。最初のコマンドで固定バージョンを直接インストールするため、Corepack は不要です。ビルド前に、起動中の Metra をすべて完全に終了してください。Metra が起動したままの場合、成果物のロックを避けるため、スクリプトは明確なメッセージを表示して停止します。

```powershell
npm install --global pnpm@11.22.0
pnpm install --frozen-lockfile
pnpm run build:portable
```

`pnpm-workspace.yaml` の `packages` にはルートパッケージ `.` が明示的に含まれています。これにより、pnpm がルートの Metra パッケージを正しく認識し、`packages field missing or empty` エラーを回避します。プロジェクトの Cargo 設定ではユーザー側の rustc wrapper も無効化されるため、グローバルな `sccache` は使用されません。ポータブル版は `src-tauri/target/release/Metra-<version>-portable.exe` に出力されます。Windows インストーラーが必要な場合は `pnpm run build` を使用してください。成果物は `src-tauri/target/release/bundle` に出力されます。

### macOS Universal

macOS の Universal ビルドには、専用コマンドを使用してください。

```text
pnpm run build:macos-universal
# npm を使用する場合
npm run build:macos-universal
```

このコマンドは `cargo` と `rustc` を同じ rustup stable ツールチェーンに固定し、`aarch64-apple-darwin` と `x86_64-apple-darwin` の両ターゲットをインストールして、`sccache` を無効にしたうえで Tauri の Universal ビルドを実行します。

`pnpm run build -- --target universal-apple-darwin` は使用しないでください。pnpm では余分な `--` が下位のコマンドへ渡り、Rust が `universal-apple-darwin` を存在しない単一ターゲットとして受け取るため、ビルドに失敗します。また、Homebrew の `cargo` / `rustc` が PATH 上で rustup より優先されている状態で、rustup にインストールしたターゲットと混在させても失敗します。上記の専用コマンドは、どちらの問題も自動的に回避します。

## 更新確認とインストール方法（v0.1.39 以降）

Windows ポータブル版を含む対応配布形式は、起動後まもなく GitHub の最新 Release にある同じ `latest.json` を読み込み、実行中は 24 時間ごとに再確認します。Windows NSIS インストール版では、Tauri updater が対応するインストーラー URL と署名を読み取ります。手動ダウンロード版は最上位のバージョンと任意の説明だけを読み取り、検証済みバージョンから固定の GitHub Release ページを組み立てるため、偽のアップデートペイロードは不要です。GitHub に接続できない場合、Metra は接続エラーを表示したり連続して再試行したりせず、次回の定期確認まで静かに待機します。新しいバージョンが見つかると、バージョン番号と更新確認を表示します。選択された更新操作は、ユーザーが明示的に承認した後にのみ開始されます。

v0.1.38 自体にはアップデーターが含まれていないため、既存のインストール版とポータブル版のユーザーは最初に v0.1.39 へ手動で更新する必要があります。Windows の項目だけを含むマニフェストでも v0.1.39 ポータブル版には通知できますが、v0.1.39 の macOS 版は Darwin のアップデートペイロードを要求します。そのため macOS は v0.1.40 へ一度だけ手動更新する必要があります。v0.1.40 以降はマニフェストのバージョンを独立して読み取るため、Darwin インストーラー項目がなくても後続リリースを検出できます。

Windows の NSIS インストール版は Tauri updater の署名を検証し、インストーラーをダウンロードしてアプリ内で更新します。macOS の更新ボタンは検出したバージョンの GitHub Release ページを開き、ユーザーが Universal DMG を手動でダウンロードします。現在の macOS 成果物には Developer ID 署名も公証もありません。公式 Release からダウンロードして SHA-256 ファイルを確認し、Metra を `/Applications` にコピーした後、Gatekeeper に阻止された場合は `xattr -cr /Applications/Metra.app` を実行してください。Apple の認証情報がなく、上流の [Tauri updater セキュリティ問題 #3505](https://github.com/tauri-apps/plugins-workspace/issues/3505) も未解決の間は、リスクのあるアプリの直接置換を無効にします。Windows ポータブル版も新バージョンを自動検出しますが、実行中のプログラムを自動でダウンロードまたは置換することはありません。更新ボタンは検出したバージョンの Release ページを開き、ユーザーが新しいポータブル EXE を手動でダウンロードします。

将来 Apple Developer ID の認証情報を利用できるようになったら、まず署名・公証済みで、引き続き DMG から手動インストールする橋渡し版を公開します。macOS updater の安全問題が修正され、置換失敗とロールバックの検証が完了した後、次のリリースで同じ `latest.json` に `darwin-aarch64` と `darwin-x86_64` を追加し、両方から 1 つの署名済み Universal `.app.tar.gz` を参照します。URL の変更や 2 つ目のマニフェストは不要です。

## 任意：公式 Claude Code API の使用量

Anthropic の Claude Code Analytics API で使用できるのは、組織レベルの Admin API キー（`sk-ant-admin...`）のみです。通常の Claude API キー（`sk-ant-api...`）では過去の使用量を照会できず、個人アカウントは Admin API を利用できません。詳細は [Claude Code Analytics API のドキュメント](https://platform.claude.com/docs/en/manage-claude/claude-code-analytics-api) を参照してください。

Metra を起動するプロセスの環境に、次の変数を設定します。

```text
ANTHROPIC_ADMIN_KEY=sk-ant-admin...
METRA_CLAUDE_API_KEY_NAME=Claude Code Key
```

`METRA_CLAUDE_API_KEY_NAME` は Anthropic Console の API キー名と一致させる必要があります。1 日分のレポートに API キーアクターが 1 つしかない場合は省略できますが、複数ある場合は必須です。Claude Code Analytics が返すのはキー ID ではなくキー名なので、組織内で一意の名前を付けてください。名前が曖昧な場合、Metra は他のキーの使用量が混入するのを防ぐため、結果を表示しません。どちらかの環境変数を変更した後は、Metra を再起動してください。

API は UTC の暦日単位で使用量を集計し、データには最大で約 1 時間の遅延が生じることがあります。また、Pro / Max の残りの割合ではなく、トークン数と推定コストが返されます。Admin API キーには組織レベルの権限があるため、信頼できるデバイスでのみ、OS の環境またはシークレットマネージャーから注入し、定期的にローテーションしてください。Metra には、これらのキーを平文で永続化する機能はありません。

## データとプライバシー

- SQLite の設定に保存されるのは、更新間隔、インターフェース言語の設定、バブル全体の位置、自動吸着の切り替え、プロバイダーの順序と表示状態、カスタムラベルと色、自動起動の設定、互換モードへの同意だけです。半隠れ状態の一時的な座標は保存されません。
- Codex の認証情報は、インストール済みの Codex CLI / App Server によって管理されます。Metra が直接読み取ったり保存したりすることはありません。
- Cursor 個人互換モードは初期設定では無効です。有効にすると、Cursor の `state.vscdb` を変更せずに読み取ります。トークンは 1 回のリクエスト中だけメモリに保持され、その後消去されます。
- Cursor へのネットワークリクエストは、`api2.cursor.sh` と `cursor.com` の HTTPS エンドポイントだけに制限され、オリジンをまたぐリダイレクトは拒否されます。
- Claude Code の収集処理では、ログイン状態を確認するコマンドだけを実行し、`~/.claude/projects` 内の JSONL ファイルからタイムスタンプ、メッセージ ID、使用量の数値をデシリアライズします。メッセージ本文、API キー、ベース URL は読み取りません。
- `ANTHROPIC_ADMIN_KEY` は Rust プロセス内で一時的に使用されるだけです。SQLite への書き込み、WebView への送信、ログへの記録、Metra が起動するサブプロセスへの引き渡しは行われません。公式リクエスト先は `https://api.anthropic.com` に固定され、リダイレクトは拒否されます。レスポンス内のユーザーアクター情報は無視され、選択した API キーの集計値だけがキャッシュされます。
- Metra は、メールアドレス、トークン、メッセージ内容、API レスポンス全体をログに記録しません。

## リリース署名

`v*` タグを push すると、リリース成果物ワークフローが起動します。現在のワークフローは Windows NSIS updater と、手動ダウンロード用の Windows ポータブル版および未署名 Universal macOS DMG を生成します。macOS Runner はバージョン、両アーキテクチャ、DMG コンテナ、SHA-256 ファイルを検証しますが、Apple の認証情報がない状態で Developer ID 署名、Gatekeeper の信頼、公証済みであるとは扱いません。

アップデート成果物には、対応する署名鍵のペアが必要です。秘密鍵は絶対にリポジトリへコミットせず、GitHub Actions の `TAURI_SIGNING_PRIVATE_KEY` Secret にアップロードするとともに、別途安全なオフラインバックアップを保管してください。暗号化した鍵を使う場合だけ、パスワードを `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` に保存します。公開鍵は `src-tauri/tauri.conf.json` に保存します。秘密鍵を紛失すると、既存クライアントは以後のアップデートを受け入れられません。現在この鍵で署名するのは Windows NSIS updater だけです。DMG と Windows ポータブル実行ファイルは手動ダウンロード用であり、updater のプラットフォーム URL には使用しません。

将来、正式な macOS 自動更新を有効にする際は、`APPLE_CERTIFICATE`、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_ID`、`APPLE_PASSWORD`、`APPLE_TEAM_ID` が追加で必要です。その時点の橋渡し版ワークフローでは、`arm64` + `x86_64` の Universal アプリ、`Developer ID Application`、hardened runtime、Gatekeeper の通過、有効な公証チケットを、Darwin updater 項目を公開する必須条件にします。

CI は最初に GitHub Release をドラフトとして作成し、Windows インストーラーと署名、手動ダウンロード用パッケージ、単一の `latest.json` をアップロードしてから、その Windows プラットフォーム URL、バージョン、署名、必須成果物がすべて一致することを検証します。すべての検証に合格した場合に限り、CI はドラフトを公開して latest に設定します。これにより、クライアントが作成途中のアップデートを検出することはありません。

未署名または ad-hoc の macOS ビルドは、現在は手動ダウンロード互換チャネルとして提供します。Developer ID 署名と公証を追加するまでは、Metra の公式 Release からのみダウンロードし、公開された SHA-256 を確認したうえで、記載の `xattr` 手順を実行してください。

## ライセンス

MIT
