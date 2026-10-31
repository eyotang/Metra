# Metra

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · **한국어** · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md)

## 소프트웨어 소개

**AI 코딩 사용량을 한눈에.**

Cursor, Codex, Claude Code의 한도, 토큰, 비용을 데스크톱 한쪽에서 확인하세요. **macOS, Windows, Linux**를 지원합니다.

- **작업 흐름을 그대로.** 버블을 자유롭게 이동하거나 화면 가장자리에 접어 두고, 상태 표시줄 모드도 사용하세요.
- **세 도구를 한곳에서.** 제공되는 한도, 초기화 시간, 사용량을 빠르게 확인하세요.
- **취향에 맞게.** 서비스 순서, 레이블, 색상, 갱신 주기를 설정하세요. 7개 언어와 자동 감지를 지원합니다.

| 사용량 상세 | 표시 설정 |
| :---: | :---: |
| <img src="docs/assets/readme/usage-demo.png" alt="사용량 상세" width="320"> | <img src="docs/assets/readme/settings-demo.png" alt="표시 설정" width="320"> |

현재 인터페이스를 가상 사용량으로 보여 줍니다. 실제 계정 정보는 포함하지 않습니다. 표시되는 정보는 서비스와 로그인 방식에 따라 다릅니다.

## 사용 방법

**[최신 버전 다운로드](https://github.com/eyotang/Metra/releases/latest)**

| 시스템 | 파일 | 설치 방법 |
| --- | --- | --- |
| macOS | `*-macos-universal.dmg` | DMG를 열고 Metra를 Applications로 드래그하세요. |
| Windows x64 | `*-windows-x64-setup.exe` | 설치 프로그램을 실행하세요. 포터블 EXE는 WebView2가 필요합니다. |
| Linux x64 | `*-linux-x64.deb` | `sudo apt install ./Metra-*-linux-x64.deb`로 설치하세요. |

macOS: Applications로 옮긴 뒤 처음 실행하기 전에 다음 명령으로 격리 속성을 제거하세요.

```sh
sudo xattr -dr com.apple.quarantine /Applications/Metra.app
```

Metra를 실행하고 사용하는 도구에 로그인한 다음 버블을 클릭하면 사용량이 표시됩니다. 정확한 Cursor 사용량이 필요하면 설정에서 **Cursor 호환 모드**를 켜고 로컬 인증 정보 읽기에 동의하세요. 드래그로 이동하고 자동 고정을 켜면 화면 가장자리에 접어 둘 수 있습니다.

설정에서 언어, 갱신 주기, 자동 시작을 변경하세요. 패널을 닫아도 백그라운드에서 실행됩니다. 종료는 설정이나 트레이 메뉴를 사용하세요.

[Linux 설정](docs/linux.md) · [Omarchy 위젯](integrations/omarchy/README.md)

## 개발 및 기여 가이드

**Node.js 24+**, **안정 버전 Rust**와 아래 시스템 도구를 준비하세요. 저장소를 Fork하고 자신의 Fork를 복제한 뒤 루트에서 실행하세요.

| 시스템 | 개발 도구 |
| --- | --- |
| macOS | Xcode 명령줄 도구 |
| Windows | Visual Studio C++ 빌드 도구, Windows SDK, WebView2 |
| Linux | WebKitGTK 4.1, GTK 3, AppIndicator. 플로팅 창은 X11/XWayland 필요 |

```sh
npm ci
npm run dev
```

PR 제출 전 확인:

```sh
npm run check
cargo test --locked --all-targets
npx playwright install chromium webkit
npm run test:ui
```

UI는 `frontend/`, 수집 로직은 `src/providers/`, 갱신 로직은 `src/service.rs`에서 시작하세요. `METRA_CONFIG_DIR`로 테스트 설정을 분리할 수 있습니다.

PR은 한 가지 문제에 집중하고 이유, 변경 내용, 검증 방법을 설명하세요. UI 변경에는 스크린샷을 첨부하고, 관련 번역과 README는 7개 언어를 함께 갱신하세요.

macOS 로컬 설치는 `npm run build` 후 `npm run install:local`을 실행하세요.

[빌드·패키징·수집기 참고](docs/development.md) · [Issues](https://github.com/eyotang/Metra/issues) · [Pull requests](https://github.com/eyotang/Metra/pulls)

[MIT 라이선스.](LICENSE)
