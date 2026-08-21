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
- 인터페이스는 영어, 중국어 간체(`zh-CN`), 일본어, 한국어를 지원합니다. “자동 감지”는 운영체제 또는 브라우저 언어를 따르며, 수동 선택은 즉시 적용되고 재시작 후에도 유지됩니다.

<p align="center">
  <a href="docs/assets/readme/metra-readme-hero.png">
    <img src="docs/assets/readme/metra-readme-hero-1920.jpg" alt="AI 사용량 버블과 개인 정보가 가려진 Cursor, Codex 및 Claude Code 사용량 패널을 보여 주는 Metra 홍보 이미지" width="100%">
  </a>
</p>

## 개발

Rust stable, Node.js 22 이상, npm 및 현재 플랫폼용 Tauri 시스템 종속성이 필요합니다.

```text
npm install
npm run check
npm run verify:i18n
npm test
npm run dev
```

릴리스 빌드를 생성하려면 다음 명령을 실행합니다.

```text
npm run build
```

Windows 빌드 결과물은 `src-tauri/target/release`와 `src-tauri/target/release/bundle`에 생성됩니다. macOS Universal 빌드를 생성하려면 다음 명령을 실행합니다.

```text
rustup target add x86_64-apple-darwin aarch64-apple-darwin
npm run build -- --target universal-apple-darwin
```

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

`v*` 태그를 푸시하면 릴리스 아티팩트 워크플로가 실행됩니다. macOS를 공식 배포하려면 릴리스 환경에 `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`를 설정해야 합니다.

이 워크플로는 macOS 산출물에 대해 `arm64`와 `x86_64`를 모두 포함한 Universal 바이너리, `Developer ID Application` 서명, hardened runtime 플래그, Gatekeeper 통과, 그리고 `.app` 및 `.dmg`의 notarization ticket 검증을 모두 강제합니다. 하나라도 빠지면 바로 실패합니다.

로컬 ad-hoc macOS 빌드는 아키텍처와 패키징 점검에는 유용하지만, 테스트 전용이며 공개 배포에는 사용할 수 없습니다.

## 라이선스

MIT
