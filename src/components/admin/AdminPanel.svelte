<script lang="ts">
import { onMount } from "svelte";
import { adminButton, adminPrimary, adminField } from "./ui";
import AdminIcon from "./AdminIcon.svelte";
import SiteSettingsManager from "./SiteSettingsManager.svelte";
import FriendsManager from "./FriendsManager.svelte";
import PagesManager from "./PagesManager.svelte";
import CommentsManager from "./CommentsManager.svelte";
import Dashboard from "./Dashboard.svelte";
import VisitsManager from "./VisitsManager.svelte";
import UsersManager from "./UsersManager.svelte";
import ArticleManager from "./ArticleManager.svelte";
import BackgroundManager from "./BackgroundManager.svelte";
import MusicManager from "./MusicManager.svelte";
import GalleryManager from "./GalleryManager.svelte";
import TalksManager from "./TalksManager.svelte";
import {
	apiFetch,
	fetchMe,
	login,
	logout,
	type AuthUser,
} from "@/utils/api-client";
interface Doc {
	id: number;
	source_key: string;
	title: string;
	url: string;
	visibility: string;
	revision: number;
	index_status: string;
	index_error: string | null;
	updated_at: string;
}
interface Detail extends Doc {
	markdown: string;
	category: string;
	tags: string[];
}
interface ImageRow {
	id: number;
	name: string;
	url: string;
	width: number;
	height: number;
	bytes: number;
	created_at: string;
}
let siteSettingsManager: SiteSettingsManager;
let friendsManager: FriendsManager;
let pagesManager: PagesManager;
let commentsManager: CommentsManager;
let articleManager: ArticleManager;
let backgroundManager: BackgroundManager;
let usersManager: UsersManager;
let musicManager: MusicManager;
let galleryManager: GalleryManager;
let talksManager: TalksManager;
let user: AuthUser | null = null;
let checking = true;
let account = "";
let password = "";
let section:
	| "settings" | "friends" | "pages" | "comments"
    | "dashboard"
	| "visits"
	| "users"
	| "articles"
	| "music"
	| "talks"
	| "gallery"
	| "backgrounds"
	| "documents"
	| "images" = "dashboard";
let docs: Doc[] = [];
let images: ImageRow[] = [];
let query = "";
let busy = false;
let message = "";
let error = "";
let editing: Detail | null = null;
let tagText = "";
let dirty = false;
let confirmDelete: {
	kind: "documents" | "images";
	id: number;
	title: string;
} | null = null;
const field = adminField;
const button = adminButton;
$: filteredDocs = docs.filter((d) =>
	`${d.title} ${d.source_key} ${d.index_status}`
		.toLowerCase()
		.includes(query.toLowerCase()),
);
$: filteredImages = images.filter((i) =>
	i.name.toLowerCase().includes(query.toLowerCase()),
);
function fail(e: unknown) {
	error = e instanceof Error ? e.message : "操作失败，请重试";
}
async function load() {
	if (section === "documents") docs = await apiFetch<Doc[]>("/documents");
	else if (section === "images")
		images = await apiFetch<ImageRow[]>("/admin/images");
}
async function run(action: () => Promise<void>) {
	if (busy) return;
	busy = true;
	error = "";
	message = "";
	try {
		await action();
	} catch (e) {
		fail(e);
	} finally {
		busy = false;
	}
}
async function signIn() {
	await run(async () => {
		await login(account, password);
		password = "";
		user = await fetchMe();
		if (user.is_admin) await load();
	});
}
async function signOut() {
	if (section === "talks" && talksManager && !talksManager.canLeave()) return;
	if (section === "gallery" && galleryManager && !galleryManager.canLeave()) return;
	if (section === "music" && musicManager && !musicManager.canLeave()) return;
	if (section === "users" && usersManager && !usersManager.canLeave()) return;
	if (
		section === "backgrounds" &&
		backgroundManager &&
		!backgroundManager.canLeave()
	)
		return;
	if (section === "articles" && articleManager && !articleManager.canLeave())
		return;
	if (!leaveEditor()) return;
	await run(async () => {
		await logout();
		user = null;
		docs = [];
		images = [];
	});
}
function leaveEditor() {
    if (section === "settings" && siteSettingsManager && !siteSettingsManager.canLeave()) return false;
    if (section === "friends" && friendsManager && !friendsManager.canLeave()) return false;
    if (section === "pages" && pagesManager && !pagesManager.canLeave()) return false;
    if (section === "comments" && commentsManager && !commentsManager.canLeave()) return false;
	if (dirty && !window.confirm("有未保存的修改，确定离开？")) return false;
	editing = null;
	dirty = false;
	return true;
}
async function switchSection(next: typeof section) {
	if (section === "talks" && talksManager && !talksManager.canLeave()) return;
	if (section === "gallery" && galleryManager && !galleryManager.canLeave()) return;
	if (section === "music" && musicManager && !musicManager.canLeave()) return;
	if (section === "users" && usersManager && !usersManager.canLeave()) return;
	if (
		section === "backgrounds" &&
		backgroundManager &&
		!backgroundManager.canLeave()
	)
		return;
	if (section === "articles" && articleManager && !articleManager.canLeave())
		return;
	if (!leaveEditor()) return;
	section = next;
	query = "";
	await run(load);
}
function create() {
	if (section === "articles" && articleManager && !articleManager.canLeave())
		return;
	if (!leaveEditor()) return;
	editing = {
		id: 0,
		source_key: `manual:${crypto.randomUUID()}`,
		title: "",
		url: "/posts/",
		markdown: "",
		category: "",
		tags: [],
		visibility: "private",
		revision: 0,
		index_status: "private",
		index_error: null,
		updated_at: "",
	};
	tagText = "";
}
async function edit(id: number) {
	if (section === "articles" && articleManager && !articleManager.canLeave())
		return;
	if (!leaveEditor()) return;
	await run(async () => {
		editing = await apiFetch<Detail>(`/documents/${id}`);
		tagText = editing.tags.join(", ");
	});
}
async function save() {
	const doc = editing;
	if (!doc) return;
	await run(async () => {
		await apiFetch("/documents", {
			method: "POST",
			body: JSON.stringify({
				source_key: doc.source_key,
				title: doc.title,
				url: doc.url,
				markdown: doc.markdown,
				category: doc.category,
				tags: tagText
					.split(/[,，]/)
					.map((t) => t.trim())
					.filter(Boolean),
				visibility: doc.visibility,
			}),
		});
		dirty = false;
		editing = null;
		await load();
		message = "文档已保存，搜索索引已更新";
	});
}
async function reindex(doc: Doc) {
	await run(async () => {
		const result = await apiFetch<{ semantic_configured: boolean }>(
			`/documents/${doc.id}/reindex`,
			{ method: "POST" },
		);
		await load();
		message = result.semantic_configured
			? "已加入语义索引队列"
			: "已加入队列，配置 embedding 服务后生成向量";
	});
}
async function remove() {
	const target = confirmDelete;
	if (!target) return;
	await run(async () => {
		await apiFetch(
			target.kind === "documents"
				? `/documents/${target.id}`
				: `/admin/images/${target.id}`,
			{ method: "DELETE" },
		);
		confirmDelete = null;
		await load();
		message = "已删除";
	});
}
async function upload(event: Event) {
	const input = event.currentTarget as HTMLInputElement;
	const file = input.files?.[0];
	if (!file) return;
	await run(async () => {
		if (file.size > 8 * 1024 * 1024) throw new Error("图片不能超过 8 MiB");
		const form = new FormData();
		form.append("file", file);
		await apiFetch("/admin/images", { method: "POST", body: form });
		await load();
		message = "图片已上传";
	});
	input.value = "";
}
async function copy(url: string, markdown = false) {
	await run(async () => {
		await navigator.clipboard.writeText(
			markdown ? `![](${url})` : new URL(url, location.origin).href,
		);
		message = markdown ? "Markdown 图片代码已复制" : "图片链接已复制";
	});
}
const statuses: Record<string, string> = {
	queued: "等待索引",
	indexing: "正在索引",
	ready: "索引就绪",
	failed: "索引失败",
	private: "私有文档",
};
onMount(() => {
	void (async () => {
		try {
			user = await fetchMe();
			if (user.is_admin) await load();
		} catch {
			user = null;
		} finally {
			checking = false;
		}
	})();
	const before = (e: BeforeUnloadEvent) => {
		if (dirty) {
			e.preventDefault();
			e.returnValue = "";
		}
	};
	window.addEventListener("beforeunload", before);
	return () => window.removeEventListener("beforeunload", before);
});
</script>

{#if checking}
 <main class="grid min-h-screen place-items-center text-slate-500">正在验证管理员身份…</main>
{:else if !user?.is_admin}
 <main class="grid min-h-screen place-items-center p-6">
  <section class="w-full max-w-sm rounded-2xl border border-slate-200 bg-[#fffdfa]/90 p-8 shadow-[0_4px_20px_rgba(74,111,165,0.07)]">
   <a href="/" class="text-sm text-indigo-600">← 返回博客</a>
   <h1 class="mt-8 text-2xl font-normal">内容管理</h1><p class="mt-2 text-sm text-slate-500">使用本站管理员账号登录</p>
   {#if user}<p class="mt-5 rounded-2xl bg-amber-50 p-3 text-sm text-amber-800">当前账号没有管理权限。</p><button class={`${button} mt-4`} onclick={signOut} disabled={busy}>退出当前账号</button>
   {:else}<form class="mt-6 space-y-4" onsubmit={(e) => { e.preventDefault(); void signIn(); }}>
    <label class="block text-sm">用户名或邮箱<input class={`${field} mt-2`} bind:value={account} autocomplete="username" required /></label>
    <label class="block text-sm">密码<input class={`${field} mt-2`} type="password" bind:value={password} autocomplete="current-password" required /></label>
    <button class={`${adminPrimary} w-full`} disabled={busy}>{busy ? "登录中…" : "登录后台"}</button>
   </form>{/if}
   {#if error}<p role="alert" class="mt-4 text-sm text-red-600">{error}</p>{/if}
  </section>
 </main>
{:else}
 <div class="min-h-screen md:flex">
  <aside class="border-b border-[#4a6fa5]/10 bg-[#faf8f5]/90 p-6 md:fixed md:inset-y-0 md:z-10 md:flex md:w-64 md:flex-col md:overflow-hidden md:border-r md:px-5 md:py-5">
   <a href="/" class="flex items-center gap-3 rounded-2xl p-2 text-[#4a6fa5] transition-all duration-500 hover:shadow-[0_6px_24px_rgba(74,111,165,0.12)]"><span aria-hidden="true" class="relative h-11 w-11"><span class="absolute left-0 top-0 h-8 w-8 rounded-full bg-[#85cdca]/55 blur-[2px]"></span><span class="absolute bottom-0 right-0 h-8 w-8 rounded-full bg-[#e8a87c]/50 blur-[2px]"></span><span class="absolute right-0 top-0 h-6 w-6 rounded-full bg-[#c38d94]/40 blur-[2px]"></span></span><span class="text-2xl font-normal italic tracking-wide">Yukina</span></a>
   <p class="ml-2 mt-3 text-sm text-[#60758b]">写作 · 图像 · 灵感</p>
   <nav aria-label="后台导航" class="admin-navigation mt-5 grid grid-cols-2 gap-1 md:mt-5 md:flex md:min-h-0 md:flex-1 md:flex-col md:overflow-y-auto">
    {#each [{key:"dashboard",title:"仪表盘"},{key:"visits",title:"访问记录"},{key:"users",title:"用户管理"},{key:"articles",title:"文章管理"},{key:"talks",title:"说说管理"},{key:"music",title:"音乐管理"},{key:"gallery",title:"相册管理"},{key:"backgrounds",title:"背景图管理"},{key:"documents",title:"搜索文档"},{key:"images",title:"图片素材"},{key:"settings",title:"站点设置"},{key:"friends",title:"友链管理"},{key:"pages",title:"页面内容"},{key:"comments",title:"评论管理"}] as item}
     <button aria-current={section === item.key ? "page" : undefined} class={`group flex shrink-0 items-center gap-3 rounded-2xl border px-3 py-1.5 text-left text-sm font-normal transition-all duration-500 ease-in-out enabled:hover:shadow-[0_8px_32px_rgba(74,111,165,0.2)] enabled:active:scale-[0.98] focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-[#4a6fa5] disabled:opacity-40 ${section === item.key ? "border-[#85cdca]/40 bg-linear-to-br from-[#85cdca]/25 to-[#e8a87c]/15 text-[#365b78] shadow-[0_4px_20px_rgba(74,111,165,0.08)]" : "border-transparent text-[#536f89] enabled:hover:border-[#4a6fa5]/15 enabled:hover:bg-[#85cdca]/15"}`} disabled={busy} onclick={() => switchSection(item.key as typeof section)}><span class={`grid h-7 w-7 shrink-0 place-items-center rounded-full transition-colors duration-500 ${section === item.key ? "bg-white/70" : "bg-[#4a6fa5]/5 group-hover:bg-white/80"}`}><AdminIcon name={item.key}/></span>{item.title}</button>
    {/each}
   </nav>
   <div class="mt-3 flex shrink-0 items-center justify-between gap-3 border-t border-[#4a6fa5]/10 px-2 pt-3 text-sm"><p class="min-w-0 truncate text-slate-500" title={user.username}>{user.username}</p><button class="shrink-0 rounded-xl px-2 py-2 text-slate-500 hover:bg-[#c38d94]/15 hover:text-red-600 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#4a6fa5]" disabled={busy} onclick={signOut}>退出登录</button></div>
  </aside>
  <main class="min-w-0 flex-1 p-5 md:ml-64 md:p-9 lg:p-12">
   <header class="relative mb-8 flex flex-wrap items-center justify-between gap-5 overflow-hidden rounded-3xl border border-[#4a6fa5]/10 bg-linear-to-br from-[#85cdca]/15 via-[#fffdfa]/70 to-[#e8a87c]/15 px-6 py-7 shadow-[0_4px_24px_rgba(74,111,165,0.04)] md:px-8 md:py-9"><div aria-hidden="true" class="pointer-events-none absolute -right-5 -top-16 h-64 w-72 rounded-full bg-[#85cdca]/15 blur-3xl"></div><div class="relative"><p class="mb-3 text-sm text-[#60758b]">我的创作工作室 / {section === "settings" ? "站点配置" : section === "dashboard" || section === "visits" || section === "users" ? "站点运营" : section === "backgrounds" || section === "images" || section === "gallery" ? "视觉素材" : "文字与内容"}</p><h1 class="text-3xl font-normal italic tracking-wide text-[#3d5b80] md:text-4xl">{section === "settings" ? "站点设置" : section === "friends" ? "友链管理" : section === "pages" ? "页面内容" : section === "comments" ? "评论管理" : section === "dashboard" ? "仪表盘" : section === "visits" ? "访问记录" : section === "users" ? "用户管理" : section === "talks" ? "说说管理" : section === "gallery" ? "相册管理" : section === "music" ? "音乐管理" : section === "articles" ? "文章管理" : section === "backgrounds" ? "背景图管理" : section === "documents" ? "搜索文档" : "图片素材"}</h1><p class="mt-4 max-w-2xl text-sm leading-7 text-[#536b83]">{section === "settings" ? "管理站点信息、导航、外观和侧栏，让每一处展示都随心调整。" : section === "friends" ? "整理朋友的博客与订阅，分享值得相遇的内容。" : section === "pages" ? "编辑关于我、留言页等页面正文，排版和样式由模板统一处理。" : section === "comments" ? "集中查看读者的留言，及时回复与整理。" : section === "dashboard" ? "站点的每一次相遇，都在这里留下真实的记录。" : section === "visits" ? "查看页面访问、访客来源与时间。" : section === "users" ? "管理账号资料、角色与登录状态。" : section === "talks" ? "记录日常与想法，管理每一条说说和分享。" : section === "gallery" ? "整理生活的片刻，管理相册、封面与照片。" : section === "music" ? "为博客挑选喜欢的旋律，管理本地歌单和 Meting 解析。" : section === "articles" ? "让灵感落在纸上，慢慢写成值得分享的故事。" : section === "backgrounds" ? "为不同的主题与设备，挑选恰到好处的风景。" : section === "documents" ? "整理文字，让每一份记录都能被再次找到。" : "收藏创作里的色彩，为文章和背景挑选画面。"}</p></div><a href="/" class={`${button} relative self-start md:self-center`}>访问博客 <span aria-hidden="true">↗</span></a></header>
   {#if error}<p role="alert" class="mb-4 rounded-2xl border border-red-200 bg-red-50 p-3 text-sm text-red-700">{error}</p>{/if}
   {#if message}<p role="status" class="mb-4 rounded-2xl border border-emerald-200 bg-emerald-50 p-3 text-sm text-emerald-700">{message}</p>{/if}
   {#if section === "dashboard"}<Dashboard onnavigate={next=>{void switchSection(next);}} />
{:else if section === "settings"}<SiteSettingsManager bind:this={siteSettingsManager} />
{:else if section === "friends"}<FriendsManager bind:this={friendsManager} />
{:else if section === "pages"}<PagesManager bind:this={pagesManager} />
{:else if section === "comments"}<CommentsManager bind:this={commentsManager} />
{:else if section === "visits"}<VisitsManager />
{:else if section === "users"}<UsersManager viewerId={user.id} bind:this={usersManager} />
{:else if section === "articles"}<ArticleManager bind:this={articleManager} />
{:else if section === "talks"}<TalksManager bind:this={talksManager} />
{:else if section === "gallery"}<GalleryManager bind:this={galleryManager} />
{:else if section === "music"}<MusicManager bind:this={musicManager} />
{:else if section === "backgrounds"}<BackgroundManager bind:this={backgroundManager} />
{:else if editing}
    <form class="rounded-3xl border border-slate-200 bg-[#fffdfa]/90 p-5 md:p-7" oninput={() => { dirty = true; }} onsubmit={(e) => { e.preventDefault(); void save(); }}>
     <div class="mb-6 flex items-center justify-between"><h2 class="text-lg font-medium">{editing.id ? "编辑文档" : "新建文档"}</h2><button type="button" class={button} disabled={busy} onclick={leaveEditor}>关闭</button></div>
     {#if editing.source_key.startsWith("astro-posts:")}<p class="mb-5 rounded-2xl bg-amber-50 p-3 text-sm text-amber-800">这篇文档由源文件同步，后续同步可能覆盖这里的修改。长期修改请同时更新源文件。</p>{/if}
     <fieldset disabled={busy} class="grid gap-5 md:grid-cols-2">
      <label class="text-sm">标题<input class={`${field} mt-2`} bind:value={editing.title} maxlength="200" required /></label>
      <label class="text-sm">原文链接<input class={`${field} mt-2`} bind:value={editing.url} placeholder="/posts/example/" required /></label>
      <label class="text-sm">来源标识<input class={`${field} mt-2`} bind:value={editing.source_key} readonly={!!editing.id} required /></label>
      <label class="text-sm">可见性<select class={`${field} mt-2`} bind:value={editing.visibility}><option value="private">私有 · 仅管理员可见</option><option value="public">公开 · 参与搜索</option></select></label>
      <label class="text-sm">分类<input class={`${field} mt-2`} bind:value={editing.category} /></label>
      <label class="text-sm">标签（逗号分隔）<input class={`${field} mt-2`} bind:value={tagText} /></label>
      <label class="text-sm md:col-span-2">Markdown 正文<textarea class={`${field} mt-2 min-h-96 resize-y font-mono leading-6`} bind:value={editing.markdown} required spellcheck="false"></textarea></label>
     </fieldset>
     <div class="mt-5 flex items-center justify-between"><p class="text-xs text-slate-400">{editing.markdown.length.toLocaleString()} 字符 · 保存后自动更新搜索索引</p><button class="rounded-2xl bg-indigo-600 px-5 py-2 text-sm font-medium text-white disabled:opacity-50" disabled={busy}>{busy ? "保存中…" : "保存文档"}</button></div>
    </form>
   {:else}
    <section class="rounded-3xl border border-slate-200 bg-white">
     <div class="flex flex-wrap items-center gap-3 border-b border-slate-100 p-5"><input class={`${field} max-w-sm`} aria-label="筛选内容" bind:value={query} placeholder={section === "documents" ? "搜索标题、来源或索引状态…" : "搜索图片名称…"} /><button class={button} disabled={busy} onclick={() => run(load)}>刷新</button><div class="ml-auto">{#if section === "documents"}<button class="rounded-2xl bg-indigo-600 px-4 py-2 text-sm font-medium text-white disabled:opacity-50" disabled={busy} onclick={create}>＋ 新建文档</button>{:else}<label class="inline-block cursor-pointer rounded-2xl bg-indigo-600 px-4 py-2 text-sm font-medium text-white">＋ 上传图片<input type="file" class="sr-only" accept="image/jpeg,image/png,image/webp,image/gif" disabled={busy} onchange={upload} /></label>{/if}</div></div>
     {#if section === "documents"}
      <div class="overflow-x-auto"><table class="w-full min-w-175 text-left text-sm"><thead class="bg-slate-50 text-xs text-slate-500"><tr><th class="px-5 py-3">文档</th><th class="px-5 py-3">可见性</th><th class="px-5 py-3">索引状态</th><th class="px-5 py-3">操作</th></tr></thead><tbody class="divide-y divide-slate-100">{#each filteredDocs as doc}<tr class="hover:bg-slate-50/60"><td class="max-w-xs px-5 py-4"><p class="truncate font-medium">{doc.title}</p><p class="mt-1 truncate text-xs text-slate-400">{doc.source_key}</p><p class="mt-1 text-xs text-slate-400">版本 {doc.revision} · {doc.updated_at.slice(0, 16)}</p></td><td class="px-5 py-4"><span class={`rounded-md px-2 py-1 text-xs ${doc.visibility === "public" ? "bg-emerald-50 text-emerald-700" : "bg-slate-100 text-slate-500"}`}>{doc.visibility === "public" ? "公开" : "私有"}</span></td><td class="px-5 py-4"><span class="text-slate-500">{statuses[doc.index_status] || doc.index_status}</span>{#if doc.index_error}<p class="mt-1 max-w-xs text-xs text-red-600">{doc.index_error}</p>{/if}</td><td class="px-5 py-4"><div class="flex flex-wrap gap-3"><button class="text-indigo-600 disabled:opacity-50" disabled={busy} onclick={() => edit(doc.id)}>编辑</button><button class="text-slate-500 disabled:opacity-30" disabled={busy || doc.visibility !== "public"} onclick={() => reindex(doc)}>重建索引</button><button class="text-red-500 disabled:opacity-50" disabled={busy} onclick={() => { confirmDelete = {kind: "documents", id: doc.id, title: doc.title}; }}>删除</button></div></td></tr>{/each}</tbody></table></div>
      {#if !filteredDocs.length}<p class="p-16 text-center text-sm text-slate-400">{query ? "没有匹配的文档" : "暂无文档，创建第一篇或运行文档同步"}</p>{/if}
     {:else}
      <div class="grid gap-5 p-5 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">{#each filteredImages as image}<article class="overflow-hidden rounded-3xl border border-slate-200"><a href={image.url} target="_blank" rel="noreferrer"><img src={image.url} alt={image.name} class="aspect-4/3 w-full bg-slate-100 object-contain" loading="lazy" /></a><div class="p-4"><p class="truncate text-sm font-medium" title={image.name}>{image.name}</p><p class="mt-1 text-xs text-slate-400">{image.width} × {image.height} · {Math.ceil(image.bytes / 1024)} KB</p><div class="mt-4 flex flex-wrap gap-3 text-xs"><button class="text-indigo-600" disabled={busy} onclick={() => copy(image.url)}>复制链接</button><button class="text-slate-500" disabled={busy} onclick={() => copy(image.url, true)}>Markdown</button><button class="ml-auto text-red-500" disabled={busy} onclick={() => { confirmDelete = {kind: "images", id: image.id, title: image.name}; }}>删除</button></div></div></article>{/each}</div>
      {#if !filteredImages.length}<p class="p-16 text-center text-sm text-slate-400">{query ? "没有匹配的图片" : "图库还是空的，上传第一张图片"}</p>{/if}
     {/if}
     <footer class="border-t border-slate-100 px-5 py-3 text-xs text-slate-400">显示最近 200 条中的 {section === "documents" ? filteredDocs.length : filteredImages.length} 条{busy ? " · 正在处理…" : ""}</footer>
    </section>
   {/if}
  </main>
 </div>
{/if}
{#if confirmDelete}
 <div class="fixed inset-0 z-50 grid place-items-center bg-slate-950/40 p-5"><div role="alertdialog" aria-modal="true" aria-labelledby="delete-title" tabindex="-1" class="w-full max-w-md rounded-2xl bg-[#fffdfa]/90 p-6 shadow-xl"><h2 id="delete-title" class="text-lg font-normal">确认删除？</h2><p class="mt-3 break-words text-sm text-slate-500">{confirmDelete.title}</p><p class="mt-2 text-sm text-red-600">{confirmDelete.kind === "images" ? "图片文件会被删除，引用该图片的链接将失效。" : "文档原文、切片和搜索索引将一起删除。"}</p><div class="mt-6 flex justify-end gap-3"><button class={button} disabled={busy} onclick={() => { confirmDelete = null; }}>取消</button><button class="rounded-2xl bg-red-600 px-4 py-2 text-sm text-white disabled:opacity-50" disabled={busy} onclick={remove}>确认删除</button></div></div></div>
{/if}

<style>
	.admin-navigation {
		scrollbar-width: none;
	}

	.admin-navigation::-webkit-scrollbar {
		display: none;
	}
</style>
