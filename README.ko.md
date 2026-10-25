# Metra

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · **한국어**

Metra는 **Tauri 2, TypeScript, Rust**으로 구현된 Cursor, Codex, Claude Code 사용량 모니터입니다. Windows는 WebView2, macOS는 WKWebView, Linux는 WebKitGTK로 렌더링합니다. 화면 가장자리의 32px 캡슐을 유지하며, 모든 플랫폼에서 배경 흐림 없는 불투명한 화면과 매끄러운 윤곽을 사용합니다.

기본 플로팅 버블, 드래그와 화면 가장자리 고정, 상태 표시줄 모드, 사용량 상세 정보, 서비스 순서·표시 여부·레이블·색상, 한국어를 포함한 4개 언어를 지원합니다. 표시 방식은 SQLite에 저장되어 재시작 후에도 유지됩니다. 기존 `dev.metra.desktop/settings.db`와 사용자가 동의한 Cursor 호환 모드 설정을 그대로 사용합니다.

## 개발

현재 안정 버전 Rust와 운영체제에 맞는 네이티브 개발 도구를 설치하세요.

```sh
npm ci
npm run dev
npm run dev -- -- --settings
npm run frontend:build
cargo test --locked --all-targets
npm run build -- --no-bundle -- --locked
```

`METRA_CONFIG_DIR`로 별도 테스트 설정 경로를 지정할 수 있습니다. Linux에는 X11/XWayland, WebKitGTK 4.1, GTK 3/AppIndicator가 필요합니다.

데스크톱 의존성이 없는 Omarchy 수집기도 별도로 빌드할 수 있습니다.

```sh
cargo build --release --locked --no-default-features --target-dir target/collector
target/collector/release/metra --usage-json
```

개발 및 패키징에는 Node.js 24 이상이 필요합니다. `node scripts/package-native.mjs`로 현재 운영체제용 패키지를 만들고, macOS에서는 `--universal`로 Apple Silicon 및 Intel용 DMG를 만듭니다. Windows NSIS 도구와 WebView2 설치는 Tauri가 관리합니다. 결과는 `release-assets/`에 저장됩니다.

업데이트는 기존 `latest.json` 및 서명 키와 호환됩니다. Windows 설치 버전은 사용자 요청 후 서명을 검증하고 설치 프로그램을 실행합니다. 포터블 버전, macOS, Linux는 해당 버전의 다운로드 페이지를 엽니다. 자세한 구조·빌드·배포 안내는 [English README](README.md), Omarchy 안내는 [별도 문서](integrations/omarchy/README.md)를 참고하세요.
