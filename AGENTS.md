# macOS 本地安装

交付 macOS 本地安装时，构建完成后使用 `npm run install:local`。这个入口负责备份、保留配置、每次执行 `xattr -cr`、复查隔离属性及验证签名；脚本成功后才算安装完成。
