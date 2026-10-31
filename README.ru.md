# Metra

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · **Русский**

Metra отслеживает использование Cursor, Codex и Claude Code. Приложение создано на **Tauri 2, TypeScript и Rust**. Windows использует WebView2, macOS — WKWebView, Linux — WebKitGTK. Все поверхности непрозрачные, без размытия фона, с плавными контурами.

Плавающий виджет размером 56 пикселей можно перемещать и сворачивать в капсулу размером 32 пикселя у края экрана. Наведение раскрывает виджет, щелчок или Enter открывает подробности. В строке состояния видимые сервисы сменяются каждые пять секунд. Панель показывает статус аккаунта, лимиты, время сброса, токены и стоимость. Настройки сохраняют порядок сервисов, видимость, подписи, цвета, язык, интервал обновления и автозапуск. Поддерживаются семь языков с автоматическим определением: английский, упрощённый китайский, японский, корейский, французский, немецкий и русский.

Существующие настройки в `dev.metra.desktop/settings.db` сохраняются. Чтение учётных данных Cursor требует явного согласия. При ошибке обновления последние данные остаются видны с пометкой об устаревании.

## Разработка

Установите стабильный Rust, Node.js 24+ и зависимости Tauri для своей системы: инструменты командной строки Xcode для macOS; инструменты сборки Visual Studio C++, Windows SDK и WebView2 для Windows; WebKitGTK 4.1, GTK 3 и AppIndicator для Linux (X11/XWayland для позиционирования окон).

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

`METRA_CONFIG_DIR` задаёт отдельный каталог тестовых настроек. Закрытие панели оставляет Metra работать; для выхода используйте меню. См. [настройку Linux](docs/linux.md).

Сборщик Omarchy работает без Node и зависимостей рабочего стола:

```sh
cargo build --release --locked --no-default-features --target-dir target/collector
target/collector/release/metra --usage-json
target/collector/release/metra --watch-json
```

Режим наблюдения принимает строки `refresh` через стандартный ввод и завершается при его закрытии. См. [руководство Omarchy](integrations/omarchy/README.md).

## Сборка пакетов и обновления

```sh
node scripts/package-native.mjs
# Универсальный DMG macOS для Apple Silicon и Intel:
rustup target add aarch64-apple-darwin x86_64-apple-darwin
node scripts/package-native.mjs --universal
```

Результаты сохраняются в `release-assets/`: DMG macOS, пользовательский установщик NSIS и переносимый EXE Windows, пакет Debian Linux. Переносимая версия Windows требует установленного WebView2. Поддерживается `CARGO_TARGET_DIR`.

Для локальной установки macOS соберите приложение и выполните `npm run install:local`. Скрипт создаёт резервную копию приложения, сохраняет настройки, удаляет расширенные атрибуты и проверяет подпись. Скачанное приложение перенесите в Applications и перед первым запуском выполните:

```sh
xattr -cr /Applications/Metra.app
```

Обновления используют существующие `latest.json` и открытый ключ Minisign. Установленная версия Windows по явному запросу пользователя проверяет подпись и запускает установщик. Переносимая версия, macOS и Linux открывают страницу загрузки для ручной установки.

Архитектура и проверки CI описаны в [английском README](README.md). [Лицензия MIT](LICENSE). Tauri использует MIT/Apache-2.0, шрифт Inter сохраняет лицензию OFL в `assets/fonts/`.
