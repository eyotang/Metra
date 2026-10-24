# Metra

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · **한국어**

Metra는 **Rust, GPUI, GPUI-KIT 0.8.0**으로 구현된 Cursor, Codex, Claude Code 사용량 모니터입니다. 브라우저나 WebView 없이 네이티브 UI를 렌더링합니다.

기본 플로팅 버블, 드래그와 화면 가장자리 고정, 상태 표시줄 모드, 사용량 상세 정보, 서비스 순서·표시 여부·레이블·색상, 한국어를 포함한 4개 언어를 지원합니다. 표시 방식은 SQLite에 저장되어 재시작 후에도 유지됩니다. 기존 `dev.metra.desktop/settings.db`와 사용자가 동의한 Cursor 호환 모드 설정을 그대로 사용합니다.

## 개발

현재 안정 버전 Rust와 운영체제에 맞는 네이티브 개발 도구를 설치하세요.

```sh
cargo run
cargo run -- --settings
cargo test --locked --all-targets
cargo build --release --locked
```

`METRA_CONFIG_DIR`로 별도 테스트 설정 경로를 지정할 수 있습니다. Linux에는 X11/XWayland, Vulkan 드라이버, GTK 3/AppIndicator가 필요합니다.

데스크톱 의존성이 없는 Omarchy 수집기도 별도로 빌드할 수 있습니다.

```sh
cargo build --release --locked --no-default-features --target-dir target/collector
target/collector/release/metra --usage-json
```

패키징 도우미에만 Node.js 24가 필요합니다. `node scripts/package-native.mjs`로 현재 운영체제용 패키지를 만들고, macOS에서는 `--universal`로 Apple Silicon 및 Intel용 DMG를 만듭니다. Windows 패키징에는 NSIS도 필요합니다. 결과는 `release-assets/`에 저장됩니다.

업데이트는 기존 `latest.json` 및 서명 키와 호환됩니다. Windows 설치 버전은 사용자 요청 후 서명을 검증하고 설치 프로그램을 실행합니다. 포터블 버전, macOS, Linux는 해당 버전의 다운로드 페이지를 엽니다. 자세한 구조·빌드·배포 안내는 [English README](README.md), Omarchy 안내는 [별도 문서](integrations/omarchy/README.md)를 참고하세요.
