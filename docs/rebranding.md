# Yukina 项目命名

站点标题、页面文案、主题标识、评论系统、组件和脚本名称、前端包名及 Rust 后端包名统一使用 Yukina / yukina。

- 默认主题：`yukina`；旧 `siteTheme=aemeath` 链接和已保存的旧主题值仍能打开新主题。
- 默认评论系统：`yukina`。
- 前端包：`yukina`；Rust 后端：`yukina-api`。
- 新默认数据库：`data/yukina.db`。当新文件不存在且同目录存在 `aemeath.db` 时，继续连接原数据库，保留已有数据。显式配置的数据库地址不变。
- 登录、访客和评论资料的旧浏览器存储键自动迁移到 `yukina.*`；已有新键优先，不会覆盖新值。
- 新刷新 Cookie 使用 `yukina_refresh_token`，兼容读取旧 Cookie，并在成功认证响应中清除旧 Cookie。

重新部署时需一并更新前端静态产物与后端程序。后端进程名改为 `yukina-api`，启动命令仍为 `cargo run`。

原项目 GitHub 仓库、演示站点链接、第三方许可证及 Git 历史保留真实地址和原始归属。旧名称在兼容迁移代码、这些上游引用及历史截图中仍可能出现。
