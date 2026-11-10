# Yukina API

Rust / Axum 后端。前端通过 `/api/v1` 使用现有认证、资料、评论和文章统计接口。

## Cargo workspace 与代码入口

后端按现有代码职责拆为多个 crate，依赖版本集中在根 `Cargo.toml` 的 `[workspace.dependencies]`。

| 目录 | 职责 |
| --- | --- |
| `crates/server` | 启动入口、资源初始化、业务路由组装及账号、文章、社区等模块；同时提供 `yukina_server` 库和 `yukina-api` 程序 |
| `crates/runtime` | AppState、JWT、密码工具、会话与用户缓存、限流、IP 归属地 |
| `crates/store` | SeaORM 实体、连接、迁移、文章持久化、分段与 SQLite 向量扩展注册 |
| `crates/embedding` | 向量模型请求、响应校验与向量编码 |
| `crates/email-notifier` | SMTP 邮件发送 |
| `crates/media-service` | 上传公共协议、图片校验与去重存储；音乐歌单、Meting 解析、音频和歌词上传与 Range 读取 |
| `crates/shared` | 配置、统一错误、时间、页面路径与 URL 校验、公网 HTTPS 请求 |

入口为 `crates/server/src/main.rs`，HTTP 路由组装与中间件位于 `crates/server/src/app.rs`，初始化位于 `crates/server/src/bootstrap.rs`。
账号和社区功能是 `server` 内部业务模块，路由位于 `routes/`，处理器位于 `handler/`，不单独拆 crate。
没有独立 CLI crate；站点导入和图片迁移脚本继续通过服务程序的维护参数执行。
`runtime`、`store` 等底层 crate 不依赖 `server`，避免形成循环依赖。配置类型下沉到 `shared`，不会反向引用路由代码。


仍然只部署一个 `yukina-api` 服务：crate 是编译与代码组织边界，不是独立进程。
工作区的 `config.toml`、`.env`、`data/`、`target/` 都放在 `backend/` 根目录。请在该目录运行启动命令，避免相对数据库和上传路径随工作目录变化。

```sh
cargo run -p yukina-api
# 原维护命令继续可用：
cargo run -p yukina-api -- import-site /absolute/path/site-plan.json
cargo run -p yukina-api -- migrate-article-images /absolute/path/image-plan.json
```

项目脚本使用 `backend/Cargo.toml`；API 地址、数据库结构和 `yukina-api` 可执行文件名保持兼容；产物位于 `backend/target/debug/`。
`default-members` 指向 `crates/server`，因此在 `backend/` 运行 `cargo run` 即可启动服务。

## 状态与内存

Axum 在路由组装和处理请求时会克隆 `AppState`。大对象必须使用 `Arc` 等共享句柄；
特别是 IP 查询器内部持有完整 xdb 字节数组，不能直接放进可深拷贝的状态中。
用户、验证码、刷新会话与限流记录按需存入内存并定期清理。刷新会话不落库，重启会使刷新令牌失效。
上传图片在请求时读取处理，不在启动阶段全量加载。

## 验证

在 `backend` 目录运行：

```sh
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

邮箱筛选请求头 `X-Comment-Email` 已纳入 CORS 白名单，与前端筛选功能对应。

文档同步的 `POST /api/v1/documents` 和 `POST /api/v1/documents/reconcile` 不经过通用 API 限流，仍要求管理员令牌，并保留 2 MiB 请求体限制、CORS 和请求日志。其他文档管理与搜索接口仍使用通用限流。

图片文件的 `GET /api/v1/uploads/{file}`（含 HEAD）不占用业务 API 的限流额度，避免图库同时加载多张图片时触发 429，导致缩略图加载失败。图片上传、删除和素材列表等接口仍使用通用限流。

## 文档存储与搜索

第一版支持 Markdown/MDX 原文存储、章节分段、中文关键词搜索和可配置语义检索。
MDX 作为文本解析，不在后端执行。暂不包含 PDF/Word 解析或在线文章编辑器。
原有 Astro 文章页面仍由本地内容构建，后端保存的是独立搜索副本。

### 同步现有文章

重新编译并启动后端后，在项目根目录运行：

```sh
pnpm documents:sync
```

该命令只校验源文件并预览数量，不写库，也不需要预先构建前端。执行同步需要现有管理员登录接口返回的访问令牌：

```sh
# 环境变量由终端或部署环境设置，令牌不要写进配置文件或提交到仓库。
DOCUMENTS_API_URL=http://127.0.0.1:8787
DOCUMENTS_ADMIN_TOKEN=<管理员 access_token>
pnpm documents:sync --apply
```

上述为变量名称示意，PowerShell 请使用 `$env:变量名 = "值"`，其他 shell 按自身语法设置。
同步命令直接解析 Markdown / MDX 的 frontmatter，校验发布日期和重复路径，并从源文件路径生成文章链接；不读取静态文章元数据文件。
发布日期写入后端 `documents.date`，只更新日期时不重建正文索引。前台日历直接读取 `/api/v1/articles` 的已发布文章与日期，链接进入后端阅读页。
草稿和密码文章存为 private，不会出现在公开检索或发往 embedding 服务。
同步成功后，从本地移除的 `astro-posts:` 文章在后端转为 private，避免残留可搜索内容；
其他来源的文档不受影响。重新添加文章后可再次同步恢复。正文、可见性或元信息不变则不重新索引。
同步按顺序执行，并遵守 API 限流返回的 Retry-After。

### 配置语义索引

未配置时，关键词索引在文档保存时立即更新，不需要外部服务。搜索页的“站内搜索”继续使用
原有 Pagefind；“文档搜索”调用后端，并提示当前实际检索模式。

配置 `config.toml` 的 `[search]`，或对应环境变量：

- `EMBEDDING_ENDPOINT`：完整 HTTP(S) 地址，例如服务提供的 `/v1/embeddings` 接口。
- `EMBEDDING_MODEL`：模型名称。
- `EMBEDDING_DIMENSIONS`：模型实际返回的向量维度，1–8192。
- `EMBEDDING_API_KEY`：可选 Bearer 密钥，只放环境变量。

接口请求格式是 `{"model":"模型名","input":["文本1","文本2"]}`，响应格式是
`{"data":[{"index":0,"embedding":[0.1,0.2]}]}`。按 index 重排并校验数量、维度、有限数值和非零向量。
外部服务请使用 HTTPS；配本机服务时可使用 HTTP。只有明确配置接口后，公开文档片段与查询文本
才会发送给该服务。无需在 Rust 进程加载模型。

重启后，单个后台工作任务从数据库队列取文档，一次一篇，每次最多 8 个片段，
与查询共用最多 2 个并发 embedding 请求；请求超时 30 秒，响应限制 4 MiB。
失败自动退避重试，累计 3 次后标为 failed，可通过管理员 reindex 接口再次排队。
重启会回收 indexing 状态的任务；更换 endpoint、model 或 dimensions 会重新索引，旧模型向量不参与新查询。
当前任务队列按单后端实例设计，不应由多个实例同时恢复任务。

### API

- `POST /api/v1/documents`：管理员按 source_key 幂等保存 Markdown。
- `GET /api/v1/documents`：管理员查看最近 200 篇文档的索引状态与错误。
- `GET /api/v1/documents/{id}`：管理员读取完整原文与元信息。
- `DELETE /api/v1/documents/{id}`：管理员删除文档及片段索引。
- `POST /api/v1/documents/{id}/reindex`：管理员重新生成公开文档向量。
- `POST /api/v1/documents/reconcile`：管理员提交完整 `astro-posts:` source_keys 清单，隐藏缺失项。
- `GET /api/v1/search?q=...&mode=hybrid&limit=10`：公开查询，只检索 public 文档。
- `GET /api/v1/search/status`：公开文档数量、语义服务配置状态和就绪数量。

保存示例：

```json
{
  "source_key": "manual:rust-memory",
  "title": "Rust 内存优化",
  "url": "/posts/rust-memory/",
  "markdown": "# 避免复制\n\n通过 Arc 共享大对象。",
  "category": "技术",
  "tags": ["Rust"],
  "visibility": "public"
}
```

不传 visibility 默认为 private。正文限制 1 MiB，链接必须为站内路径。搜索返回标题、
命中章节、纯文本片段、原文链接与相关度分数；关键词与向量结果用 RRF 融合，并按文档去重。
分数是排序分数，不是可信度百分比。关键词使用 FTS5 trigram；不足三个字的词回退转义后的 LIKE，
向量使用静态链接的 sqlite-vec 计算余弦距离。第一版为数据库内精确扫描，适用于当前博客规模，
不构建常驻内存 ANN 索引；数据量增长后需按实际延迟评估 vec0/独立向量服务。

### 内容管理后台

前端入口 `/admin/` 使用本站管理员账号登录，包含文章管理、背景图管理、搜索文档和图片素材。

文章管理包含标题、Slug、分类、日期、摘要、封面选图、标签、草稿/发布、置顶，以及 Vditor
即时渲染、分屏和源码编辑模式、预览、大纲、全屏、图片粘贴/拖拽上传与图库插图。
支持多文件 Markdown 导入（默认草稿）、单篇下载、选中项 ZIP 下载。文章的标题、正文、分类、标签、
发布状态、Slug、摘要、日期、封面和置顶统一存入 `documents` 的同一行，在同一事务中保存；公开列表 `/articles/`，
阅读地址 `/reader/?slug=...`，对应 `/api/v1/articles` 与 `/api/v1/articles/{slug}`。
`astro-posts:` 来源的正文仍可能在下一次源文件同步时被覆盖。
旧 `/posts/.../` 详情地址只生成页面外壳，浏览时调用 `GET /api/v1/article-source?source_key=...` 读取后端当前公开文章，
`/reader/?slug=...` 调用 `GET /api/v1/articles/{slug}`。标题、正文、摘要、封面、标签、分类和日期均以后端响应为准；
读取不缓存，失败或下架时显示错误，不回退本地正文。旧来源链接不受后台修改 Slug 影响。
文章正文图片和详情封面使用后端素材地址 `/api/v1/uploads/...`；原文章地址的评论与阅读统计标识保持不变。
AI 摘要独立保存在 `documents.ai_summary`，详情页从后端元信息读取并显示摘要卡片，留空则隐藏。
后台文章编辑、Markdown 导入和导出均支持该字段；同步脚本将 frontmatter 的 `aiSummary` 写入后端。
旧客户端未传该字段时保留已有值，显式传空字符串可清空；只修改摘要不重建正文索引。

#### 生成 AI 摘要

文章编辑页的“生成摘要”按钮使用当前编辑框中的标题和正文，支持尚未保存的新文章。
生成结果填回 AI 摘要框，标记为未保存；检查后点击“创建文章”或“更新文章”才写入数据库。
生成失败保留原摘要，生成期间正文发生变化时不覆盖编辑内容。摘要不自动发布文章。

在后端环境变量或本机 `.env` 中配置以下项目，然后重启后端：

```dotenv
AI_SUMMARY_ENDPOINT=https://你的模型服务/v1/chat/completions
AI_SUMMARY_MODEL=你的模型名称
AI_SUMMARY_API_KEY=你的服务密钥
```

`endpoint` 和 `model` 也可配置在 `config.toml` 的 `[ai_summary]`，环境变量优先；密钥仅放后端环境，
不提交仓库、不返回浏览器。使用本机模型且无需鉴权时可省略密钥；本机回环地址允许 HTTP，其余地址使用 HTTPS。
摘要模型与搜索的 embedding 模型独立配置。接口采用兼容的 Chat Completions 格式，发送 `model`、`messages`、
`max_completion_tokens` 和 `stream: false`，读取 `choices[0].message.content`，参见
[OpenAI Docs 接口格式](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)。

`POST /api/v1/admin/articles/generate-summary` 仅限管理员，正文上限 1 MiB，标题最多 200 字。
正文超过 24000 字时采用首尾各 12000 字，并在界面提示检查；服务超时 45 秒，最多同时生成 2 个摘要。
返回 `{ "summary": "...", "truncated": false }`，只返回草稿，不修改数据库。未配置模型、空正文或上游错误会明确提示。
文章列表按每页20篇分页，可按标题、分类、来源和发布状态筛选；R2 存储和迁移尚未接入，本次支持本地存储。

升级旧数据库时，启动迁移会为 `documents` 补齐文章字段，将旧 `articles` 的元信息按原 ID 合并，
在同一事务中建立 Slug 唯一索引并移除旧表；原正文、文章链接、公开状态、索引切片和关联 ID 保留。
新库不再创建 `articles` 表。普通导入资料的 `slug` 为 NULL，不会混入公开文章列表与文章搜索。
`astro-posts:` 来源明确代表博客文章：旧记录在启动迁移时补齐缺失 Slug，新同步记录在保存时自动补齐；
只补文章标识，不覆盖正文、公开状态或后台已编辑的元信息。重复执行不改变已有 Slug，重名时使用记录 ID 后缀避让。
`document_chunks` 和 `document_fts` 继续作为派生检索索引，不是第二份文章业务表。
迁移可重复执行；若旧表存在未关联正文的记录或冲突 Slug，事务回滚，旧数据保留并报告错误。
发布此版本前备份数据库，停止旧版后端后再启动新版；迁移后不可同时运行仍依赖旧表的后端。

背景管理使用 `backgrounds` 表，主题包含 light/dark/all（不限主题），设备为 desktop/mobile。
不限主题的背景同时用于亮色、暗色轮换。支持图库选图、上传、URL 添加、编辑媒体与主题/设备、
拖动排序、上下移动、批量移除，也可将本站原配置中的壁纸导入选定分组。
`PATCH /api/v1/backgrounds/{id}` 要求管理员权限，编辑保留原排序位置；
排序支持全部和筛选视图，筛选排序仅交换可见项的位置，隐藏项的位置保持不变。排序需点击保存，
离开未保存的编辑或排序时会提示确认。
视频支持 MP4/WebM、50 MiB 上限，分块写入 `data/backgrounds`，读取支持 Range 流式响应。
前台现有壁纸容器会使用对应分组的背景，每 15 秒轮换；该分组为空时保留原配置。
移除配置不会删除素材文件；上传后未添加的素材以及移除的背景视频需由运维清理。

图库接口：`GET/POST /api/v1/admin/images`、`DELETE /api/v1/admin/images/{id}`，
均要求管理员身份。图片存放在 `data/uploads/media-*.jpg`，元数据通过 SeaORM 写入
`media_images` 表。仅图库上传项可在此删除，现有头像与评论图不会混入图库。
上传上限 8 MiB，解码有尺寸和内存限制，图片转为 JPEG（动图取首帧，透明区域不保留），
最长边缩小至 4096px；图片链接公开可读，删除文件后原链接失效。
前端支持预览、复制链接和 Markdown 图片代码，删除需要二次确认。

#### 文章原图迁移与存储

`pnpm images:migrate` 预览当前本机数据库的文章图片迁移，`pnpm images:migrate --apply` 执行。
脚本需要支持 `node:sqlite` 和 `backup` 的 Node.js（本项目环境为 22.20）；执行时调用 Rust 后端迁移命令，
如运行中的后端锁住 Windows 可执行文件，请先停掉旧后端，迁移结束后再启动新版。
可通过 `--database 路径`、`--data-dir 路径` 指定数据库和后端数据目录；默认使用 `backend/data/yukina.db`，
不存在时兼容原 `backend/data/aemeath.db`。不要把图片迁移当成文章重新同步：它读取后端当前正文，不覆盖后台编辑成果。

迁移会自动备份 SQLite 数据库和迁移计划到后端数据目录的 `backups/`，原图保存在 `uploads/`，不删除前端原图。
仅替换 Markdown 中真实的图片节点或引用定义，代码示例保持不变；详情封面同步为后端素材地址，已有后端封面优先。
本地源图按文章的 `source_key` 定位，支持相对路径、public 图片和早期 `/文章目录/图片名` 写法。
原图使用 SHA-256 内容地址，重复迁移和同步不会重复创建素材；保留 PNG 透明度、WebP 和 GIF 原始字节。
无法定位的本地图片会在预览时报错并阻止迁移；外部 HTTP(S) 链接不下载，保持原地址。
所有文件就绪后在数据库事务内更新正文和封面；发现文章在迁移期间被修改时回滚文章更新，请重新预览。

`POST /api/v1/admin/images/import` 为管理员原图接口，上限 32 MiB，支持 PNG/JPEG/WebP/GIF，解码有尺寸和内存限制。
`pnpm documents:sync --apply` 会先上传本地正文图片和封面，再将后端图片地址写入正文，不再写回前端相对图片依赖。
后台编辑器上传或选图后插入的图片同样使用后端地址；迁移后的文章可以完全在后台继续维护。
单独导入 `.md` 不会自动携带旁边的图片目录，需要先上传图片并更新链接，或使用本地源文件同步命令。
仍被文章、相册、背景、说说、评论或用户资料引用的素材不能直接删除（返回 409）。

部署和恢复时同时备份数据库与 `data/uploads`，只恢复数据库不能恢复图片文件。
后端保留稳定的图片访问入口，后续接入 OSS 可替换存储实现；文章正文无需依赖前端资源映射表。

生产环境沿用现有 `/api/v1` 到 Rust 后端的反向代理；开发环境使用 Astro 已有 API 代理。

本地构建预览使用 `pnpm preview --host 127.0.0.1 --port 4333`，由 `scripts/preview.mjs` 同时转发 `/api/v1` 配置接口和上传文件到本机 8787 端口，需保持 Rust 后端运行。不要直接用 `pnpm astro preview` 预览后端功能：Astro 7 的静态预览不会加载开发代理，并可能把 API 请求作为静态页面返回 404。

背景图按后台设置的亮色／暗色和桌面／移动端范围生效；外观设置中的“纯色背景”会关闭壁纸显示。排查时先确认预览站点的 `/api/v1/backgrounds` 和其中的图片地址都能返回 200，再核对这些显示条件。

### 仪表盘、访问记录与用户管理

后台默认进入仪表盘：今日/昨日和累计 PV、UV，7/14/30 天访问趋势、热门页面、最近访问，
以及文档、用户、评论、图库、背景数量。统计使用 `visit_events` 表的数据库聚合，
不把完整访问明细加载到内存；日期边界固定为北京时间。首次上线前的历史访问不会自动补录。

公开页面通过 `POST /api/v1/analytics/events` 上报页面访问，支持 Astro/Swup 导航去重；
相同访客和事件 ID 重试不重复计数。尊重 DNT 和 Global Privacy Control，不采集后台页面。
路径去掉查询参数与锚点，来源地址去掉查询参数、锚点和账号信息；访客 ID 加盐哈希保存。
`GET /api/v1/admin/dashboard` 和 `GET /api/v1/admin/visits` 仅管理员可调用；访问记录支持
IP 精确匹配、页面/标题、开始/结束日期及分页。原始 IP、来源和 User-Agent 仅管理员可读。
PV/UV 以此功能记录的数据为准，不合并 Umami 或既有文章阅读计数；UV 按浏览器标识去重，
不是自然人数。记录持久保存在 SQLite，当前不自动清理历史访问。

用户管理接口：`GET/POST /api/v1/admin/users`、`PATCH/DELETE /api/v1/admin/users/{id}`，
以及 `POST /api/v1/admin/users/{id}/reset-password`、`.../revoke-sessions`。
支持新建、搜索、角色/状态筛选、分页、昵称/邮箱/角色/启用状态编辑、密码重置、撤销会话、删除。
不能禁用、删除或降权自己的账号，至少保留一个启用管理员；删除账号保留评论并解除关联。
邮箱与用户名保持唯一；后台不提供尚不存在的邮箱验证状态或第三方账号字段。

`users.auth_version` 启动时补列，默认 0。权限和版本每次认证直接查询数据库，用户资料仍可缓存。
角色或状态变更、密码重置、撤销会话会更新版本并撤销刷新会话，已有 JWT 立即失效。
旧 JWT 没有版本字段时按版本 0 兼容。后台列表不返回密码哈希或令牌版本。

### 音乐管理

后台“音乐管理”支持本地歌单和 Meting 两种模式，切换模式保留另一种配置。
本地歌单支持歌曲新增/编辑/移除、拖动与按钮排序、封面素材选择、音频和 LRC 上传；
还可设置默认音量、列表/单曲/随机播放和歌词显示。移除歌单歌曲不会删除已上传文件。

管理员配置接口为 `GET/PUT /api/v1/admin/music`，保存时必须携带当前 `revision`，
并发修改返回 409；`POST /api/v1/admin/music/preview` 刷新解析结果供后台试听。
配置通过 SeaORM 保存在 SQLite 的 `music_settings` 单行记录，启动时自动建表。
Meting 主接口、平台、类型、ID、备用接口可在后台设置；可选 `auth` 单独保存在后端，
读取配置只返回 `has_auth`，留空保留原值，勾选清除后删除原值。数据库备份应按含凭据的数据保管。
Meting 接口需为公网 HTTPS，后端检查 DNS 地址并固定连接地址，不跟随重定向；
主接口失败后依次请求备用接口，整个解析最多 22 秒，成功缓存 5 分钟、失败缓存 15 秒。
缓存只保存当前解析歌单，最多 500 首，接口响应上限 1 MiB；音频不会整首加载进后端内存。

`POST /api/v1/admin/music/upload` 接收 multipart 的 `file`，支持 MP3/WAV/FLAC/OGG/M4A
（最大 50 MiB）和 UTF-8 LRC（最大 512 KiB），校验文件签名后存入 `DATA_DIR/music`。
返回的 `/api/v1/music/media/{file}` 支持 HTTP Range，可播放、拖动进度；具体格式兼容性取决于浏览器。
封面使用已有图片素材上传和选择功能。上传文件和配置数据库需一同备份。

前台首次初始化读取公开的 `GET /api/v1/music/playlist`，不返回 Meting 模板或认证配置。
后台尚未保存、接口缺失或后端不可用时，沿用 `src/config/musicConfig.ts` 的公开配置；
旧的 `meting.auth` 不再注入前台 HTML，请迁移至后台认证信息输入框。
保存后刷新前台生效，不中断访客正在播放的音乐；后续歌单修改无需重新构建前端。
Meting 模式已保存但解析失败时显示错误提示，不自动切换到旧的本地歌单。

### 数据与测试

普通文档业务统一使用 SeaORM：`entities/document.rs` 和 `entities/document_chunk.rs`
映射现有表，`documents/store.rs` 负责事务内的条件 upsert、切片维护和同步清理，
`documents/indexing.rs` 通过实体查询管理索引队列及版本校验。搜索状态和短关键词 LIKE
查询也使用实体查询；仅 FTS5、sqlite-vec 专用检索及数据库初始化 DDL 保留原生 SQL。
此次实体映射兼容现有表结构，不需要清库或重新导入文档。

原文、队列、片段、向量和 FTS 均落在现有 SQLite 数据库。新表和触发器启动时幂等创建，
不修改既有用户或评论数据。备份时应使用 SQLite 在线备份或停服复制，不能忽略运行中的 WAL。

`cargo test --workspace` 覆盖中文检索、向量校验、真实 SQLx 扩展注册、模型隔离、私有文档过滤、
幂等同步、修改/删除清理、重试、管理员权限与本地模拟 embedding 的索引和查询。
以下测试需先安装前端依赖并构建；它在内存数据库和临时 HTTP 端口中调用真实同步脚本，
不会写入正在运行的数据库：

```sh
cargo test -p yukina-api sync_site_documents_through_real_http -- --ignored
```


## 站点与社区内容管理

站点配置、友链、独立页面由 `site_entries` 按条目持久化，后台支持修订号冲突检查。首次导入运行 `pnpm site:import --apply`（项目根目录），会先备份数据库，已有后台配置不会被覆盖。

公开接口新增 `/api/v1/site`、`/api/v1/site-bootstrap.js`、`/api/v1/friends`、`/api/v1/friends/feed`、`/api/v1/pages/{slug}`、`/api/v1/feed.xml` 和 `/api/v1/sitemap.xml`。管理员通过 `/api/v1/admin/site/{key}` 读取与保存，通过 `/api/v1/admin/comments` 筛选评论。页面 Markdown 使用和文章相同的安全渲染规则，原始 HTML 不执行。

部署代理、静态 SEO 更新、外观偏好和迁移说明见 [后台站点管理](../docs/managed-site.md)。更新后需要重启后端，更新前端构建产物。
