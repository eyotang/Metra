# macOS 本地安装

交付 macOS 本地安装时，构建完成后使用 `npm run install:local`。这个入口负责备份、保留配置、每次执行 `xattr -cr`、复查隔离属性及验证签名；脚本成功后才算安装完成。

# 多语言与 README

应用支持英语（en）、简体中文（zh-CN）、日语（ja）、韩语（ko）、法语（fr）、德语（de）、俄语（ru）。新增或修改界面文案时同步维护 `assets/locales/` 中全部语言，保持键名和占位符一致；新增语言时同时接通语言菜单、设置序列化和系统语言识别，并验证语言切换与设置往返保存。

README 以 `README.md` 为英语入口，其他版本为 `README.zh-CN.md`、`README.ja.md`、`README.ko.md`、`README.fr.md`、`README.de.md`、`README.ru.md`。每份 README 顶部提供完整语言导航；功能、安装、构建和更新说明发生变化时同步维护各语言版本。 README 正文统一按「软件介绍 → 使用说明 → 开发贡献指南」排列：介绍配当前界面的脱敏演示截图，使用说明优先给出最新版本下载和按系统安装步骤，贡献指南包含开发启动、验证和 PR 指引；保持简洁，详细构建与架构说明放入 `docs/development.md`。

# 发布描述

创建或编辑发布说明（`docs/releases/<tag>.md` 和 GitHub Release 正文）时，始终先写完整英语部分，再写中文部分。每次发布都保留 macOS 的 `xattr` 说明及以下命令，说明在将应用移入 Applications 后、首次启动前执行，用于移除隔离标记：

```sh
sudo xattr -dr com.apple.quarantine /Applications/Metra.app
```

发布前检查两种语言的顺序及 `xattr` 命令均存在；编辑已有草稿时同步更新 GitHub Release 正文。
