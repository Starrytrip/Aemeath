# 后台站点管理

后台新增四个入口：

- **站点设置**：基础信息、博主资料、公告、导航菜单、外观默认值、侧栏布局、页脚。图片可上传或从素材库选择。
- **友链管理**：新增、编辑、推荐、排序、上下架；填写 HTTPS RSS 地址后，朋友圈从后端更新订阅。
- **页面内容**：编辑关于我、开源项目介绍、留言页说明、友链页说明，默认可视化编辑，支持可选 Markdown 源码、图片上传、图库插入、预览、草稿与发布。
- **评论管理**：按内容、昵称或页面筛选评论，集中回复或删除。删除继续使用原有软删除机制。

配置保存到 `site_entries`。每条记录带修订号；保存冲突时需要重新加载，避免覆盖另一窗口的修改。
站点配置都是公开展示数据，不应填写 API 密钥或邮件密码。图片被配置、页面或友链引用时，素材库会阻止删除。

## 迁移与备份

首次部署运行 `pnpm site:import` 查看待导入条目，再运行 `pnpm site:import --apply`。此命令先备份数据库，再导入前端配置与四个已有页面；不会覆盖后台已保存的配置。
可用 `--database <路径>` 指定数据库，或 `--runner <后端可执行文件>` 使用预编译后端。Node.js 需支持内置 `node:sqlite`（当前项目使用 Node 22.20）。

日常备份数据库及 `backend/data/uploads`。迁移前备份和导入计划位于数据库旁的 `backups` 目录。

## 文章数据来源

首页、分类导航、归档、分类、标签、侧栏统计、搜索、日历和文章详情从后端读取；新文章通过 `/reader/?slug=...` 访问。下架文章会从这些公开入口移除。旧 `/posts/...` 仅保留路径兼容，不参与文章列表或正文的数据回退。
首页分页使用 `?page=2`，无需为后台新增的文章重新生成分页文件。

RSS 和站点地图实时从数据库生成，只包含公开文章；RSS 保留完整正文和图片：

- `/api/v1/feed.xml`
- `/api/v1/sitemap.xml`

开发服务器与 `pnpm preview` 已代理 `/rss.xml`、`/sitemap.xml` 和旧 `/sitemap-index.xml`。
正式部署静态前端时，也需配置代理。例如 Nginx：

```nginx
location /api/v1/ {
    proxy_pass http://127.0.0.1:8787;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
}
location = /rss.xml { proxy_pass http://127.0.0.1:8787/api/v1/feed.xml; }
location = /sitemap.xml { proxy_pass http://127.0.0.1:8787/api/v1/sitemap.xml; }
location = /sitemap-index.xml { return 301 /sitemap.xml; }
```

## 外观与静态 HTML

后台控制站点默认值，访客自己的主题、明暗、配色与布局选择优先。设置读取不会写入访客偏好，也不会每次切页重建樱花。修改默认主题后刷新页面生效。
侧栏可调整组件顺序、左右位置、跟随滚动、文章页显示和仅文章页显示。

前台通过 `/api/v1/site-bootstrap.js` 提前读取默认主题，再将站点资料应用到页面。普通配置保存后刷新页面或切页即可看到。
项目仍使用 Astro 静态输出：爬虫直接读取的初始 HTML、canonical 和 Open Graph 信息需要重建前端。`pnpm build` 会自动同步后台公开配置到 `src/constants/managed-site.json`，再生成静态 HTML；不需要手改配置文件。
构建机与数据库分开时设置 `SITE_API_URL=https://你的站点域名`，或设置 `SITE_DATABASE` 指向本地数据库。也可先运行 `pnpm site:export` 更新构建快照。

朋友圈按后台启用的友链 RSS 刷新，缓存约 10 分钟，每个源最多读取 20 篇。读取失败会显示失败源数量；当前仅支持直接返回 XML 的 HTTPS 订阅地址，不自动发现或跟随重定向。请求会验证公网 IP，避免访问本地服务。

打赏名单、工具推荐、账单和博客日志仍是原有前端数据，属于后续可选迁移项。


### 旧页面正文升级

`pnpm pages:clean` 可预览清理范围，`pnpm pages:clean --apply` 执行前自动备份数据库。支持 `--database <路径>`。迁移只更新页面正文、修订号和修改时间，保留标题与发布状态；并发修改会使迁移回滚。
旧装饰 HTML 与悬浮提示会被移除，正文文字、图片、链接与代码示例保留；旧 GitHub 指令转为普通链接，友链 MDX 模板转为可编辑说明。以后首次导入页面时会自动完成同样清理。
