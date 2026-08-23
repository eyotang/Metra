# Metra

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · **한국어**

Metra는 Cursor, Codex, Claude Code의 로그인 상태, 사용량 한도와 재설정 시각, 누적 토큰 및 사용 가능한 지출액을 확인할 수 있는 가벼운 크로스 플랫폼 데스크톱 버블입니다.

## 주요 기능

- Windows와 macOS에서 자유롭게 드래그할 수 있고 다중 모니터를 지원하며 항상 위에 표시되는 투명한 버블입니다. 컨텍스트 메뉴에서 선택적으로 화면 가장자리 자동 도킹을 켤 수 있으며, 기본값은 꺼짐입니다.
- 자동 도킹을 켜고 처음으로 드래그해 화면 가장자리에 놓으면 즉시 32px 너비의 부분 숨김 상태로 전환되며, 드래그 후 유휴 시간을 기다리지 않습니다. 마우스 포인터를 올리거나 포커스하거나 클릭하면 전체 버블이 즉시 나타납니다. 다시 나타난 뒤에는 평소와 같이 유휴 시간이 지난 후 부분 숨김 상태로 돌아갑니다.
- 로컬의 `cursor-agent` / `agent`, `codex`, `claude` CLI를 자동으로 찾습니다.
- 공식 Codex App Server를 사용하여 여러 한도 구간과 누적 토큰을 불러옵니다.
- 필요할 때 사용자 모르게 호환 모드를 켜거나 CLI를 설치하지 않고 Cursor의 공식 로그인 절차를 엽니다. 정확한 개인 사용량을 확인하려면 별도의 동의가 필요합니다. Ultra 요금제는 Cursor Models, Other Models, Grok Bot 주간 한도 및 On-Demand를 각각 표시하며, Team과 같은 기존 요금제는 현재의 금액 기반 레이아웃을 유지합니다.
- `claude auth status --json`으로 Claude Code 로그인 상태를 확인하고, 가능한 경우 로컬 세션의 토큰 수를 읽기 전용으로 집계합니다. Anthropic API 키로 로그인한 경우 조직 관리자가 Admin API 키를 명시적으로 설정하여 선택한 키 액터의 공식 UTC 당일 토큰 사용량과 예상 비용을 가져올 수 있습니다. API 키 또는 사용자 지정 게이트웨이가 남은 한도를 제공하지 않으면 임의의 백분율을 만들지 않고 한도 데이터가 없다고 표시합니다.
- 공급자별 표시 여부를 선택하고, 점 6개 핸들로 순서를 바꾸며, 버블 레이블과 마커 색상을 모두 사용자 지정할 수 있습니다. 기본 제공되는 55색 팔레트는 다른 설정과 함께 SQLite에 저장됩니다.
- 왼쪽 클릭으로 세부 정보를 열고 패널의 새로고침 아이콘으로 사용량을 업데이트합니다. 컨텍스트 메뉴 상단에는 언어 선택기가 있으며 새로고침 간격, 시작 시 실행, 호환 모드, CLI 다시 감지 및 종료도 설정할 수 있습니다. 새로고침 동작은 중복해서 표시하지 않습니다.
- 새로고침에 실패해도 마지막으로 성공한 결과를 유지하고 오래된 데이터임을 명확히 표시합니다.
- v0.1.39부터 Windows 포터블 버전을 포함한 지원 배포본은 실행 직후 GitHub Releases의 단일 `latest.json`을 확인하고 실행 중에는 24시간마다 다시 확인합니다. Windows NSIS 설치 버전은 승인 후 앱 안에서 업데이트하며, macOS와 Windows 포터블 버전은 같은 매니페스트로 버전을 감지한 뒤 해당 Release 페이지를 열어 직접 다운로드합니다.
- 인터페이스는 영어, 중국어 간체(`zh-CN`), 일본어, 한국어를 지원합니다. “자동 감지”는 운영체제 또는 브라우저 언어를 따르며, 수동 선택은 즉시 적용되고 재시작 후에도 유지됩니다.

<p align="center">
  <a href="docs/assets/readme/metra-readme-hero.png">
    <img src="docs/assets/readme/metra-readme-hero-1920.jpg" alt="AI 사용량 버블과 개인 정보가 가려진 Cursor, Codex 및 Claude Code 사용량 패널을 보여 주는 Metra 홍보 이미지" width="100%">
  </a>
</p>

## 개발

Rust stable, Node.js 22.13 이상, npm 및 현재 플랫폼용 Tauri 시스템 종속성이 필요합니다.

```text
npm install
npm run check
npm run verify:i18n
npm test
npm run dev
```

### Windows

Windows PowerShell에서는 `package.json`에 고정된 pnpm 11.22.0을 사용하여 다음 순서로 포터블 버전을 빌드하세요. 첫 번째 명령이 고정된 버전을 직접 설치하므로 Corepack은 필요하지 않습니다. 빌드하기 전에 실행 중인 모든 Metra 인스턴스를 완전히 종료하세요. Metra가 실행 중이면 결과물 파일 잠금을 방지하기 위해 스크립트가 명확한 메시지를 표시하고 중단됩니다.

```powershell
npm install --global pnpm@11.22.0
pnpm install --frozen-lockfile
pnpm run build:portable
```

`pnpm-workspace.yaml`의 `packages`에는 루트 패키지 `.`가 명시적으로 포함되어 있습니다. 따라서 pnpm이 루트 Metra 패키지를 올바르게 인식하며 `packages field missing or empty` 오류를 방지합니다. 프로젝트 Cargo 설정은 사용자 수준 rustc wrapper도 비활성화하므로 전역 `sccache`가 사용되지 않습니다. 포터블 버전은 `src-tauri/target/release/Metra-<version>-portable.exe`에 생성됩니다. Windows 설치 프로그램이 필요하면 `pnpm run build`를 사용하세요. 결과물은 `src-tauri/target/release/bundle`에 생성됩니다.

### macOS Universal

macOS Universal 빌드에는 전용 명령을 사용하세요.

```text
pnpm run build:macos-universal
# npm을 사용하는 경우
npm run build:macos-universal
```

이 명령은 `cargo`와 `rustc`를 동일한 rustup stable 툴체인으로 고정하고, `aarch64-apple-darwin`과 `x86_64-apple-darwin` 두 타깃을 설치한 뒤, `sccache`를 비활성화하고 Tauri Universal 빌드를 실행합니다.

`pnpm run build -- --target universal-apple-darwin`은 사용하지 마세요. pnpm에서는 불필요한 `--`가 하위 명령으로 전달되어 Rust가 `universal-apple-darwin`을 존재하지 않는 단일 타깃으로 받으므로 빌드가 실패합니다. 또한 PATH에서 Homebrew의 `cargo` / `rustc`가 rustup보다 우선하는 상태로 rustup에 설치된 타깃과 혼용해도 실패합니다. 위 전용 명령은 두 문제를 모두 자동으로 방지합니다.

## 업데이트 확인 및 설치 방식(v0.1.39 이상)

Windows 포터블 버전을 포함한 지원 배포본은 실행 직후 GitHub 최신 Release의 같은 `latest.json`을 읽고 실행 중에는 24시간마다 다시 확인합니다. Windows NSIS 설치 버전에서는 Tauri 업데이터가 일치하는 설치 프로그램 URL과 서명을 읽습니다. 수동 다운로드 배포본은 최상위 버전과 선택적 설명만 읽고 검증된 버전으로 고정 GitHub Release 페이지를 만들기 때문에 가짜 업데이트 페이로드가 필요하지 않습니다. GitHub에 연결할 수 없으면 Metra는 연결 오류를 표시하거나 연속으로 재시도하지 않고 다음 예정 확인 주기까지 조용히 기다립니다. 새 버전이 발견되면 버전 번호와 업데이트 확인 메시지를 표시하며, 사용자가 명시적으로 승인한 후에만 선택한 업데이트 작업을 시작합니다.

v0.1.38 자체에는 업데이터가 포함되어 있지 않으므로 기존 설치 버전과 포터블 버전 사용자는 먼저 v0.1.39로 직접 업데이트해야 합니다. Windows 항목만 있는 매니페스트도 v0.1.39 포터블 버전에는 알릴 수 있지만, v0.1.39 macOS는 Darwin 업데이트 페이로드를 요구합니다. 따라서 macOS는 v0.1.40으로 한 번 직접 업데이트해야 합니다. v0.1.40부터는 매니페스트 버전을 독립적으로 읽으므로 Darwin 설치 항목이 없어도 이후 릴리스를 감지할 수 있습니다.

Windows NSIS 설치 버전은 Tauri 업데이트 서명을 검증하고 설치 프로그램을 내려받아 앱 안에서 업데이트합니다. macOS의 업데이트 버튼은 감지한 버전의 GitHub Release 페이지를 열고 사용자가 Universal DMG를 직접 다운로드하게 합니다. 현재 macOS 산출물에는 Developer ID 서명이나 공증이 없습니다. 공식 Release에서 다운로드하고 SHA-256 파일을 확인한 뒤 Metra를 `/Applications`에 복사하고, Gatekeeper가 실행을 막으면 `xattr -cr /Applications/Metra.app`을 실행하세요. Apple 자격 증명이 없고 업스트림 [Tauri 업데이터 안전 문제 #3505](https://github.com/tauri-apps/plugins-workspace/issues/3505)가 해결되지 않은 동안에는 위험한 제자리 앱 교체를 비활성화합니다. Windows 포터블 버전도 새 버전을 자동으로 감지하지만 실행 중인 프로그램을 자동으로 내려받거나 교체하지 않습니다. 업데이트 버튼은 감지한 버전의 Release 페이지를 열며, 사용자가 새 포터블 EXE를 직접 다운로드합니다.

나중에 Apple Developer ID 자격 증명을 사용할 수 있게 되면 먼저 서명 및 공증되었지만 여전히 DMG에서 직접 설치하는 브리지 버전을 배포합니다. macOS 업데이터 안전 문제가 수정되고 교체 실패 및 롤백 검증이 완료되면 다음 릴리스에서 같은 `latest.json`에 `darwin-aarch64`와 `darwin-x86_64`를 추가하고, 두 항목이 하나의 서명된 Universal `.app.tar.gz`를 가리키게 합니다. 주소를 바꾸거나 두 번째 매니페스트를 도입할 필요가 없습니다.

## 선택 사항: 공식 Claude Code API 사용량

Anthropic의 Claude Code Analytics API는 조직 수준의 Admin API 키(`sk-ant-admin...`)만 받습니다. 일반 Claude API 키(`sk-ant-api...`)로는 과거 사용량을 조회할 수 없고, 개인 계정은 Admin API를 사용할 수 없습니다. [Claude Code Analytics API 문서](https://platform.claude.com/docs/en/manage-claude/claude-code-analytics-api)를 참고하세요.

Metra를 실행하는 프로세스의 환경에 다음 변수를 설정하세요.

```text
ANTHROPIC_ADMIN_KEY=sk-ant-admin...
METRA_CLAUDE_API_KEY_NAME=Claude Code Key
```

`METRA_CLAUDE_API_KEY_NAME`은 Anthropic Console의 API 키 이름과 일치해야 합니다. 일일 보고서에 API 키 액터가 하나만 있으면 생략할 수 있지만, 둘 이상이면 반드시 설정해야 합니다. Claude Code Analytics는 키 ID가 아니라 키 이름을 반환하므로 조직 내에서 고유한 이름을 사용하세요. 이름이 중의적이면 Metra는 다른 키의 사용량이 섞이는 것을 막기 위해 결과를 표시하지 않습니다. 두 환경 변수 중 하나라도 변경한 후에는 Metra를 다시 시작하세요.

API는 UTC 달력일 기준으로 사용량을 집계하며, 데이터가 최대 약 1시간 지연될 수 있습니다. 또한 Pro / Max 남은 비율이 아니라 토큰 수와 예상 비용을 보고합니다. Admin API 키에는 조직 수준의 권한이 있으므로 신뢰할 수 있는 기기에서만 운영체제 환경 또는 비밀 관리자를 통해 주입하고 정기적으로 교체하세요. Metra는 이러한 키를 평문으로 저장하는 기능을 제공하지 않습니다.

## 데이터 및 개인 정보 보호

- SQLite 설정에는 새로고침 간격, 인터페이스 언어 설정, 전체 버블 위치, 자동 도킹 여부, 공급자 순서와 표시 여부, 사용자 지정 레이블과 색상, 시작 시 실행 설정 및 호환 모드 동의만 저장됩니다. 일부 숨김 상태의 임시 좌표는 저장되지 않습니다.
- Codex 인증 정보는 설치된 Codex CLI/App Server에서 계속 관리합니다. Metra는 이를 직접 읽거나 저장하지 않습니다.
- Cursor 개인 호환 모드는 기본적으로 꺼져 있습니다. 이 모드를 켜면 Metra는 Cursor의 `state.vscdb`를 수정하지 않고 읽기만 합니다. 토큰은 한 번의 요청 동안에만 메모리에 존재하며 이후 삭제됩니다.
- Cursor 네트워크 요청은 `api2.cursor.sh`와 `cursor.com`의 HTTPS 엔드포인트로 제한되며, 교차 출처 리디렉션은 거부됩니다.
- Claude Code 정보 수집은 로그인 상태 명령만 실행하며 `~/.claude/projects` 아래의 JSONL 파일에서 타임스탬프, 메시지 ID 및 사용량 수치를 역직렬화합니다. 메시지 본문, API 키 또는 기본 URL은 읽지 않습니다.
- `ANTHROPIC_ADMIN_KEY`는 Rust 프로세스 내에서만 일시적으로 사용됩니다. SQLite에 기록하거나 WebView로 보내거나 로그에 남기거나 Metra가 시작한 하위 프로세스에 전달하지 않습니다. 공식 요청은 `https://api.anthropic.com`으로 고정되고 리디렉션은 거부됩니다. 응답의 사용자 액터 정보는 무시하며 선택한 API 키의 집계 수치만 캐시합니다.
- Metra는 이메일 주소, 토큰, 메시지 내용 또는 전체 API 응답을 로그에 기록하지 않습니다.

## 릴리스 서명

`v*` 태그를 푸시하면 릴리스 아티팩트 워크플로가 실행됩니다. 현재 워크플로는 Windows NSIS 업데이터와 수동 다운로드용 Windows 포터블 및 서명되지 않은 Universal macOS DMG를 생성합니다. macOS Runner는 버전, 두 아키텍처, DMG 컨테이너와 SHA-256 파일을 검증하지만, Apple 자격 증명이 없는 상태에서 Developer ID 서명, Gatekeeper 신뢰 또는 공증을 주장하지 않습니다.

업데이트 산출물에는 서로 일치하는 서명 키 쌍이 필요합니다. 개인 키는 절대로 저장소에 커밋하지 말고 GitHub Actions의 `TAURI_SIGNING_PRIVATE_KEY` Secret에 업로드하는 동시에 별도의 안전한 오프라인 백업을 보관하세요. 키를 암호화한 경우에만 비밀번호를 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`에 저장하며, 공개 키는 `src-tauri/tauri.conf.json`에 보관합니다. 개인 키를 잃으면 설치된 클라이언트가 이후 업데이트를 받을 수 없습니다. 현재 이 키는 Windows NSIS 업데이터만 서명합니다. DMG와 Windows 포터블 실행 파일은 수동 다운로드 자산이며 updater 플랫폼 URL로 사용하지 않습니다.

나중에 정식 macOS 자동 업데이트를 활성화하려면 `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`가 추가로 필요합니다. 그때의 브리지 워크플로는 `arm64` + `x86_64` Universal 앱, `Developer ID Application`, hardened runtime, Gatekeeper 통과 및 유효한 공증 티켓을 Darwin 업데이터 항목 게시의 필수 조건으로 적용합니다.

CI는 먼저 GitHub Release를 초안으로 만들고 Windows 설치 프로그램과 서명, 수동 다운로드 패키지 및 단일 `latest.json`을 업로드한 다음 Windows 플랫폼 URL, 버전, 서명 및 필수 산출물이 모두 일치하는지 검증합니다. 모든 검증을 통과한 후에만 초안을 게시하고 latest 릴리스로 지정하므로, 클라이언트가 아직 완성되지 않은 업데이트를 발견하지 않습니다.

서명되지 않았거나 ad-hoc인 macOS 빌드는 현재 수동 다운로드 호환 채널로 제공합니다. Developer ID 서명과 공증을 추가하기 전에는 Metra 공식 Release에서만 내려받고 공개된 SHA-256을 확인한 뒤 문서의 `xattr` 단계를 따르세요.

## 라이선스

MIT
