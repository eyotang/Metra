# Metra

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · **Русский**

## О приложении

**Использование инструментов ИИ для кода — с первого взгляда.**

Лимиты, токены и стоимость Cursor, Codex и Claude Code в небольшом виджете на рабочем столе. Для **macOS, Windows и Linux**.

- **Всегда под рукой.** Перемещайте виджет, закрепляйте у края экрана или используйте строку состояния.
- **Три инструмента в одном месте.** Проверяйте доступные лимиты, время сброса и использование.
- **Настройте под себя.** Меняйте порядок сервисов, подписи, цвета и интервал обновления. Семь языков с автоматическим определением.

| Данные использования | Настройки отображения |
| :---: | :---: |
| <img src="docs/assets/readme/usage-demo.png" alt="Данные использования" width="320"> | <img src="docs/assets/readme/settings-demo.png" alt="Настройки отображения" width="320"> |

Текущий интерфейс с демонстрационными данными, без реальных сведений об аккаунтах. Доступные данные зависят от сервиса и способа входа.

## Как начать

**[Скачать последнюю версию](https://github.com/eyotang/Metra/releases/latest)**

| Система | Файл | Установка |
| --- | --- | --- |
| macOS | `*-macos-universal.dmg` | Откройте DMG и перенесите Metra в Applications. |
| Windows x64 | `*-windows-x64-setup.exe` | Запустите установщик; переносимый EXE требует WebView2. |
| Linux x64 | `*-linux-x64.deb` | Установите: `sudo apt install ./Metra-*-linux-x64.deb`. |

В macOS после переноса в Applications, перед первым запуском выполните команду для удаления расширенных атрибутов, включая карантин:

```sh
xattr -cr /Applications/Metra.app
```

Запустите Metra, войдите в используемые инструменты и нажмите на виджет. Для точных данных Cursor включите **совместимость Cursor** в настройках и разрешите чтение локальных учётных данных. Перетаскивайте виджет; автозакрепление сворачивает его у края экрана.

В настройках меняются язык, интервал обновления и автозапуск. Закрытие панели оставляет Metra работать; для выхода используйте настройки или меню в трее.

[Настройка Linux](docs/linux.md) · [Виджет Omarchy](integrations/omarchy/README.md)

## Разработка и участие

Установите **Node.js 24+**, **стабильный Rust** и инструменты ниже. Создайте форк репозитория, клонируйте его и выполните в корне:

| Система | Инструменты |
| --- | --- |
| macOS | Инструменты командной строки Xcode |
| Windows | Инструменты сборки Visual Studio C++, Windows SDK, WebView2 |
| Linux | WebKitGTK 4.1, GTK 3, AppIndicator; X11/XWayland для плавающих окон |

```sh
npm ci
npm run dev
```

Перед отправкой PR:

```sh
npm run check
cargo test --locked --all-targets
npx playwright install chromium webkit
npm run test:ui
```

Интерфейс: `frontend/`; сбор данных: `src/providers/`; обновление: `src/service.rs`. `METRA_CONFIG_DIR` позволяет отделить тестовые настройки.

Один PR — одна задача: опишите проблему, изменение и проверку. Для изменений интерфейса приложите скриншоты; обновите затронутые переводы и README на всех семи языках.

Локальная установка macOS: `npm run build`, затем `npm run install:local`.

[Справочник по сборке, пакетам и сборщику данных](docs/development.md) · [Issues](https://github.com/eyotang/Metra/issues) · [Pull requests](https://github.com/eyotang/Metra/pulls)

[Лицензия MIT.](LICENSE)
