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
- インターフェースは英語、簡体字中国語（`zh-CN`）、日本語、韓国語に対応しています。「自動検出」は OS またはブラウザの言語に従い、手動で選んだ言語はすぐに反映され、再起動後も保持されます。

<p align="center">
  <a href="docs/assets/readme/metra-readme-hero.png">
    <img src="docs/assets/readme/metra-readme-hero-1920.jpg" alt="AI 使用量バブルと、プライバシーに配慮して情報を伏せた Cursor、Codex、Claude Code の使用量パネルを示す Metra のプロモーション画像。" width="100%">
  </a>
</p>

## 開発

Rust stable、Node.js 22 以降、npm、および使用するプラットフォーム向けの Tauri システム依存関係が必要です。

```text
npm install
npm run check
npm run verify:i18n
npm test
npm run dev
```

リリースビルドを作成するには、次を実行します。

```text
npm run build
```

Windows の成果物は `src-tauri/target/release` と `src-tauri/target/release/bundle` に出力されます。macOS の Universal ビルドには、専用コマンドを使用してください。

```text
pnpm run build:macos-universal
# npm を使用する場合
npm run build:macos-universal
```

このコマンドは `cargo` と `rustc` を同じ rustup stable ツールチェーンに固定し、`aarch64-apple-darwin` と `x86_64-apple-darwin` の両ターゲットをインストールして、`sccache` を無効にしたうえで Tauri の Universal ビルドを実行します。

`pnpm run build -- --target universal-apple-darwin` は使用しないでください。pnpm では余分な `--` が下位のコマンドへ渡り、Rust が `universal-apple-darwin` を存在しない単一ターゲットとして受け取るため、ビルドに失敗します。また、Homebrew の `cargo` / `rustc` が PATH 上で rustup より優先されている状態で、rustup にインストールしたターゲットと混在させても失敗します。上記の専用コマンドは、どちらの問題も自動的に回避します。

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

`v*` タグを push すると、リリース成果物ワークフローが起動します。macOS を正式配布するには、リリース環境に `APPLE_CERTIFICATE`、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_ID`、`APPLE_PASSWORD`、`APPLE_TEAM_ID` を設定する必要があります。

このワークフローは macOS 成果物に対して、`arm64` と `x86_64` の Universal バイナリ、`Developer ID Application` 署名、hardened runtime フラグ、Gatekeeper の審査通過、そして `.app` と `.dmg` の notarization ticket 検証を必須条件として強制します。どれか 1 つでも欠けると失敗します。

ローカルの ad-hoc macOS ビルドは、アーキテクチャやパッケージングの確認には有用ですが、テスト専用であり一般公開には使えません。

## ライセンス

MIT
