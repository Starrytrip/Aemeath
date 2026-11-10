# Rust 规范检查记录

检查范围：`backend/` 的工作区依赖、错误边界、SQLite 迁移、上传写入、异步任务、配置及现有测试。
规范来源：用户提供的 `2026-11-09-AGENTS.md` 与 `2026-11-09-RUST.md`，均已去重写入根目录 `AGENTS.md`，项目原有指南保留。

## 已修正

- 数据库迁移：用事务内的表结构查询判断缺列，删除对 `duplicate column` 错误消息的匹配；真实 SQL 失败向上传递，重复执行保留已有数据。
- 错误边界：数据库、文件系统和密码库错误保留原始 source；HTTP 响应增加稳定的 kind，兼容已有 detail；500 的内部信息仅记录到日志，保留 429 的 Retry-After。
- 数据库连接：不再将连接及 WAL 错误格式化为字符串后丢弃原因。
- 音乐和视频上传：在目标目录创建临时文件，完成格式校验、同步写入后发布正式文件；提前返回或取消时由所有权清理临时文件。采用 [tempfile 官方实现](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html)，避免自行维护临时文件生命周期。
- 删除错误类型和管理员提取器上已无必要的 allow(dead_code)。没有新增全局 lint 策略。

## 保留的现有设计及边界

- 保留现有 server 二进制名称、入口和 crate 划分。指南针对“新 binary”的规则不用于重拆现有项目。
- shared 当前包含 HTTP 错误、配置与公网请求，不是纯数据/wire-type crate。后续若抽取纯数据类型，应避免引入这些运行时依赖；本次不新增类型层。
- 现有启动流程仍通过 expect 报告不可恢复的启动失败；索引和缓存清理任务目前与进程同寿命，尚未提供独立的优雅关闭接口。
- 存量模块仍有 String 配置错误及将依赖错误转换为 API 文本的路径；本次修正了共用错误边界和数据库连接，没有为统一风格全量重写所有模块。
- 不批量增加 newtype、feature、测试辅助 crate 或依赖层。上述规范明确要求避免为机械达标重写正常代码。

## 验证

- `cargo check --offline --manifest-path backend/Cargo.toml --workspace`
- `cargo test --offline --manifest-path backend/Cargo.toml --workspace`
- `cargo clippy --offline --manifest-path backend/Cargo.toml --workspace --all-targets -- -D warnings`
- `cargo fmt --manifest-path backend/Cargo.toml --all --check`
- 单独输出验收程序，避免覆盖正在运行的 yukina-api；在临时 SQLite 与数据目录实测登录、音乐/视频上传中断清理、正常发布及读取原始字节。

没有修改生产数据库、媒体地址或前台页面。
