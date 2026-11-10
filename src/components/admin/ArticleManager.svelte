<script lang="ts">
import { onMount } from "svelte";
import { adminButton, adminPrimary, adminField } from "./ui";
import JSZip from "jszip";
import { parse, stringify } from "yaml";
import { apiFetch } from "@/utils/api-client";
import MarkdownEditor from "./MarkdownEditor.svelte";
interface Doc {
	id: number;
	title: string;
	source_key: string;
	url: string;
	visibility: string;
	index_status: string;
}
interface Meta {
	id: number;
	slug: string;
	description: string;
	ai_summary: string;
	date: string;
	cover_url: string;
	is_pinned: boolean;
}
interface Form extends Doc {
	markdown: string;
	category: string;
	tags: string[];
}
let docs: Doc[] = [];
let metadata: Meta[] = [];
let current: Form | null = null;
let meta: Meta;
let busy = false;
let generatingSummary = false;
let summaryRequest: AbortController | null = null;
let error = "";
let message = "";
let query = "";
let page = 0;
let visibility = "";
let tag = "";
let dirty = false;
let selected: number[] = [];
let editor: MarkdownEditor;
let picker: "cover" | "body" | null = null;
let images: { id: number; url: string; name: string }[] = [];
let deletion: Doc | null = null;
const field = adminField;
const btn = adminButton;
const primary = adminPrimary;
$: rows = [...docs].sort(
	(a, b) =>
		Number(metadata.find((m) => m.id === b.id)?.is_pinned || false) -
		Number(metadata.find((m) => m.id === a.id)?.is_pinned || false),
);

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
	for (let attempt = 0; ; attempt++) {
		try {
			return await apiFetch<T>(path, init);
		} catch (e) {
			if (
				!(e instanceof Error) ||
				!("status" in e) ||
				e.status !== 429 ||
				attempt >= 30
			)
				throw e;
			await new Promise((resolve) => setTimeout(resolve, 1500));
		}
	}
}
async function run(fn: () => Promise<void>) {
	if (busy) return;
	busy = true;
	error = "";
	message = "";
	try {
		await fn();
	} catch (e) {
		error = e instanceof Error ? e.message : "操作失败";
	} finally {
		busy = false;
	}
}
async function load() {
	[docs, metadata] = await Promise.all([
		request<Doc[]>(
			`/documents?offset=${page * 20}&limit=20&q=${encodeURIComponent(query)}&visibility=${visibility}`,
		),
		request<Meta[]>("/admin/articles"),
	]);
}
export function canLeave() {
	return !dirty || confirm("有未保存修改，确定离开？");
}
function leave() {
	if (dirty && !confirm("有未保存修改，确定返回？")) return false;
	dirty = false;
	current = null;
	return true;
}
function slug(text: string) {
	return (
		text
			.toLowerCase()
			.replace(/[^\p{L}\p{N}]+/gu, "-")
			.replace(/^-|-$/g, "")
			.slice(0, 150) || `article-${Date.now()}`
	);
}
function defaults(doc: Doc): Meta {
	return {
		id: doc.id,
		slug: slug(doc.title),
		description: "",
		ai_summary: "",
		date: new Date().toISOString().slice(0, 10),
		cover_url: "",
		is_pinned: false,
	};
}
function create() {
	if (!leave()) return;
	current = {
		id: 0,
		title: "",
		source_key: `manual:${crypto.randomUUID()}`,
		url: "/reader/",
		visibility: "private",
		index_status: "private",
		markdown: "",
		category: "",
		tags: [],
	};
	meta = defaults(current);
}
async function edit(doc: Doc) {
	if (!leave()) return;
	await run(async () => {
		current = await request<Form>(`/documents/${doc.id}`);
		meta = { ...defaults(doc), ...metadata.find((m) => m.id === doc.id) };
	});
}
function addTag() {
	const t = tag.trim();
	if (t && current && !current.tags.includes(t)) {
		current.tags = [...current.tags, t];
		dirty = true;
	}
	tag = "";
}
async function save() {
	if (!current) return;
	const doc = current;
	await run(async () => {
		if (!doc.id && meta.slug.startsWith("article-"))
			meta.slug = slug(doc.title);
		await request("/admin/articles", {
			method: "POST",
			body: JSON.stringify({ document: doc, ...meta }),
		});
		dirty = false;
		current = null;
		await load();
		message = "文章已保存，公开文章可通过阅读页和搜索访问";
	});
}
async function generateSummary() {
	if (busy || !current) return;
	const doc = current;
	const title = doc.title;
	const markdown = doc.markdown;
	const previousSummary = meta.ai_summary;
	if (!title.trim() || !markdown.trim()) {
		error = "请先填写文章标题和正文。";
		return;
	}
	generatingSummary = true;
	await run(async () => {
		const controller = new AbortController();
		summaryRequest = controller;
		try {
			const result = await apiFetch<{ summary: string; truncated: boolean }>(
				"/admin/articles/generate-summary",
				{
					method: "POST",
					body: JSON.stringify({ title, markdown }),
					signal: AbortSignal.any([
						controller.signal,
						AbortSignal.timeout(60000),
					]),
				},
			);
			if (
				current !== doc ||
				doc.title !== title ||
				doc.markdown !== markdown ||
				meta.ai_summary !== previousSummary
			) {
				throw new Error("生成期间文章内容已更改，请重新生成摘要。");
			}
			meta.ai_summary = result.summary;
			dirty = true;
			message = result.truncated
				? "正文较长，已根据首尾片段生成摘要，请检查后保存文章。"
				: "摘要已生成，请检查后保存文章。";
		} finally {
			summaryRequest = null;
		}
	});
	generatingSummary = false;
}
async function openPicker(target: typeof picker) {
	await run(async () => {
		images = await request("/admin/images");
		picker = target;
	});
}
function choose(url: string) {
	if (picker === "cover") meta.cover_url = url;
	else editor.insert(`![](${url})\n`);
	dirty = true;
	picker = null;
}
function download(blob: Blob, name: string) {
	const url = URL.createObjectURL(blob);
	const a = document.createElement("a");
	a.href = url;
	a.download = name;
	a.click();
	setTimeout(() => URL.revokeObjectURL(url), 1000);
}
async function exportDocs(ids: number[]) {
	await run(async () => {
		const zip = new JSZip();
		for (const id of ids) {
			const doc = await request<Form>(`/documents/${id}`);
			const m = metadata.find((m) => m.id === id) || defaults(doc);
			zip.file(
				`${m.slug}.md`,
				`---\n${stringify({ title: doc.title, ...m, category: doc.category, tags: doc.tags, is_draft: doc.visibility !== "public" })}---\n\n${doc.markdown}`,
			);
		}
		if (ids.length === 1) {
			const [name, file] = Object.entries(zip.files)[0];
			download(await file.async("blob"), name);
		} else download(await zip.generateAsync({ type: "blob" }), "articles.zip");
	});
}
async function importFiles(event: Event) {
	const input = event.currentTarget as HTMLInputElement;
	const files = Array.from(input.files || []);
	await run(async () => {
		for (const file of files) {
			if (file.size > 1024 * 1024) throw Error(`${file.name} 超过1MiB`);
			const text = await file.text();
			const match = text.match(/^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/);
			const data = match ? parse(match[1]) || {} : {};
			const title = String(data.title || file.name.replace(/\.mdx?$/i, ""));
			const s = slug(String(data.slug || title));
			await request("/admin/articles", {
				method: "POST",
				body: JSON.stringify({
					document: {
						source_key: `import:${s}`,
						title,
						markdown: match ? text.slice(match[0].length) : text,
						category: String(data.category || ""),
						tags: Array.isArray(data.tags) ? data.tags.map(String) : [],
						visibility: "private",
						url: "/reader/",
					},
					slug: s,
					description: String(data.description || ""),
					ai_summary: String(data.ai_summary || data.aiSummary || ""),
					date: String(data.date || new Date().toISOString()).slice(0, 10),
					cover_url: String(data.cover_url || data.image || ""),
					is_pinned: !!data.is_pinned,
				}),
			});
		}
		await load();
		message = `已导入 ${files.length} 篇为草稿，请检查后发布`;
	});
	input.value = "";
}
onMount(() => {
	void run(load);
	const before = (e: BeforeUnloadEvent) => {
		if (dirty) {
			e.preventDefault();
			e.returnValue = "";
		}
	};
	window.addEventListener("beforeunload", before);
	return () => {
		window.removeEventListener("beforeunload", before);
		summaryRequest?.abort();
	};
});
</script>
{#if error}<p role="alert" class="mb-4 rounded-3xl bg-red-50 p-4 text-sm text-red-700">{error}</p>{/if}
{#if message}<p role="status" class="mb-4 rounded-3xl bg-teal-50 p-4 text-sm text-teal-800">{message}</p>{/if}
{#if current}
 <form oninput={()=>dirty=true} onsubmit={e=>{e.preventDefault();void save();}}>
 <header class="mb-5 flex flex-wrap justify-between gap-3"><button class={btn} type="button" onclick={leave}>← 返回列表</button><div class="flex gap-3"><button type="button" class={btn} onclick={()=>{if(current){current.visibility=current.visibility==="public"?"private":"public";dirty=true;}}}>{current.visibility==="public"?"发布":"草稿"}</button><button type="button" class={btn} onclick={()=>{meta.is_pinned=!meta.is_pinned;dirty=true;}}>{meta.is_pinned?"已置顶":"置顶"}</button><button class={primary} disabled={busy}>{current.id?"更新文章":"创建文章"}</button></div></header>
 <fieldset disabled={busy} class="grid gap-5 rounded-2xl border border-slate-200 bg-[#fffdfa]/90 p-6 shadow-[0_4px_20px_rgba(74,111,165,0.07)] md:grid-cols-2">
 <label class="text-sm text-slate-500">标题<input class={`${field} mt-2`} bind:value={current.title} required maxlength="200" onblur={()=>{if(!current?.id&&meta.slug.startsWith("article-"))meta.slug=slug(current?.title||"");}} /></label>
 <label class="text-sm text-slate-500">Slug<input class={`${field} mt-2`} bind:value={meta.slug} required /></label>
 <label class="text-sm text-slate-500">分类<input class={`${field} mt-2`} bind:value={current.category}/></label>
 <label class="text-sm text-slate-500">日期<input class={`${field} mt-2`} type="date" bind:value={meta.date} required/></label>
 <label class="text-sm text-slate-500 md:col-span-2">摘要<textarea class={`${field} mt-2 min-h-28`} bind:value={meta.description}></textarea></label>
 <div class="text-sm text-slate-500 md:col-span-2"><div class="flex items-center justify-between gap-3"><label for="article-ai-summary">AI摘要</label><button type="button" class={btn} disabled={busy || !current.title.trim() || !current.markdown.trim()} onclick={generateSummary}>{generatingSummary ? "正在生成…" : "✦ 生成摘要"}</button></div><textarea id="article-ai-summary" class={`${field} mt-2 min-h-28`} bind:value={meta.ai_summary} maxlength="3000" placeholder="填写或生成 AI 摘要，留空则不显示摘要卡片。"></textarea><p class="mt-2 text-xs text-slate-400">根据当前标题和正文生成，检查后保存文章。</p></div>
 <div class="md:col-span-2"><p class="mb-2 text-sm text-slate-500">封面图</p><div class="flex flex-col gap-4 sm:flex-row"><div class="grid h-36 w-full sm:w-48 shrink-0 place-items-center overflow-hidden rounded-3xl bg-slate-50 text-sm text-slate-400">{#if meta.cover_url}<img class="h-full w-full object-cover" src={meta.cover_url} alt="封面预览"/>{:else}未设置封面{/if}</div><div class="min-w-0 flex-1 space-y-2"><button type="button" class={btn} onclick={()=>openPicker("cover")}>从已上传图片选择</button><button type="button" class={btn} onclick={()=>{meta.cover_url="";dirty=true;}}>清空</button><input class={field} bind:value={meta.cover_url} aria-label="封面图URL" placeholder="也可手动填写封面图 URL"/></div></div></div>
 <div class="md:col-span-2"><p class="mb-2 text-sm text-slate-500">标签</p><div class="flex flex-wrap items-center gap-2">{#each current.tags as t}<span class="rounded-2xl bg-teal-50 px-3 py-2 text-sm text-teal-800">{t}<button type="button" aria-label={`移除标签 ${t}`} class="ml-2" onclick={()=>{if(current)current.tags=current.tags.filter(v=>v!==t);dirty=true;}}>×</button></span>{/each}<input class={`${field} max-w-52`} bind:value={tag} placeholder="输入标签后回车" onkeydown={e=>{if(e.key==="Enter"){e.preventDefault();addTag();}}}/></div></div>
 {#if current.source_key.startsWith("astro-posts:")}<p class="text-sm text-amber-700 md:col-span-2">源文件同步可能覆盖正文修改，请同步维护源文件。</p>{/if}
 </fieldset>
 <section class="mt-5 overflow-hidden rounded-2xl border border-slate-200 bg-[#fffdfa]/90 p-5 shadow-[0_4px_20px_rgba(74,111,165,0.07)]"><h2 class="mb-4 font-medium">正文内容 <span class="float-right text-xs text-slate-400">图片存储：本地</span></h2><MarkdownEditor bind:this={editor} value={current.markdown} onchange={text=>{if(current){current.markdown=text;dirty=true;}}} onlibrary={()=>openPicker("body")}/></section>
 </form>
{:else}
 <div class="mb-5 flex flex-wrap justify-between gap-3"><input class={`${field} max-w-sm`} bind:value={query} placeholder="搜索标题、分类、来源…" aria-label="搜索文章" onkeydown={e=>{if(e.key==="Enter"){page=0;void run(load);}}}/><div class="flex gap-3"><label class={btn}>↑ 导入 Markdown<input class="sr-only" type="file" accept=".md,.mdx" multiple onchange={importFiles} disabled={busy}/></label><button class={primary} onclick={create} disabled={busy}>＋ 新建文章</button></div></div>
 <div class="mb-4 flex gap-3"><select class={btn} aria-label="发布状态筛选" bind:value={visibility} onchange={()=>{page=0;selected=[];void run(load);}}><option value="">全部状态</option><option value="public">已发布</option><option value="private">草稿</option></select><button class={btn} disabled={busy} onclick={()=>{page=0;void run(load);}}>搜索</button></div>
<section class="overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-[0_4px_20px_rgba(74,111,165,0.07)]"><div class="flex items-center gap-4 border-b border-slate-100 p-4 text-sm"><span>已选择 {selected.length} 篇</span><button class={btn} disabled={!selected.length||busy} onclick={()=>exportDocs(selected)}>↓ 打包下载</button><button class={btn} disabled={busy} onclick={()=>run(load)}>刷新</button><a class="ml-auto text-teal-700" href="/articles/" target="_blank" rel="noreferrer">查看已发布文章 ↗</a></div>
 <div class="overflow-x-auto"><table class="w-full min-w-160 text-left text-sm"><thead class="bg-slate-50 text-slate-500"><tr><th class="p-4">选择</th><th>文章</th><th>日期</th><th>状态</th><th>操作</th></tr></thead><tbody>{#each rows as doc}<tr class="border-t border-slate-100"><td class="p-4"><input aria-label={`选择 ${doc.title}`} type="checkbox" value={doc.id} bind:group={selected}/></td><td class="max-w-sm py-5"><p class="font-medium">{doc.title}{#if metadata.find(m=>m.id===doc.id)?.is_pinned}<span class="ml-2 text-xs text-rose-600">置顶</span>{/if}</p><p class="mt-1 text-xs text-slate-400">{doc.source_key}</p></td><td>{metadata.find(m=>m.id===doc.id)?.date||"—"}</td><td><span class="rounded-md bg-slate-50 px-2 py-1">{doc.visibility==="public"?"已发布":"草稿"}</span></td><td><div class="flex gap-3"><button class="rounded-2xl px-2 py-1 text-teal-700 hover:bg-[#85cdca]/25 hover:shadow-[0_6px_24px_rgba(74,111,165,0.2)]" disabled={busy} onclick={()=>edit(doc)}>编辑</button><button disabled={busy} onclick={()=>exportDocs([doc.id])}>下载</button><button class="rounded-2xl px-2 py-1 text-rose-600 hover:bg-[#c38d94]/15" onclick={()=>deletion=doc}>删除</button></div></td></tr>{/each}</tbody></table></div>{#if !rows.length}<p class="p-14 text-center text-slate-400">暂无文章</p>{/if}<footer class="flex items-center justify-between p-4 text-xs text-slate-400"><span>第 {page+1} 页 · 每页20篇</span><div class="flex gap-3"><button class={btn} disabled={busy||page===0} onclick={()=>{page--;selected=[];void run(load);}}>上一页</button><button class={btn} disabled={busy||docs.length<20} onclick={()=>{page++;selected=[];void run(load);}}>下一页</button></div></footer></section>
{/if}
{#if picker}<div class="fixed inset-0 z-50 grid place-items-center bg-slate-900/40 p-6"><section class="max-h-[80vh] w-full max-w-3xl overflow-auto rounded-2xl bg-[#fffdfa]/90 p-6"><div class="mb-4 flex justify-between"><h2 class="font-normal">选择已上传图片</h2><button onclick={()=>picker=null}>关闭 ×</button></div><div class="grid grid-cols-3 gap-3">{#each images as img}<button onclick={()=>choose(img.url)}><img class="aspect-video w-full rounded-2xl object-cover" src={img.url} alt={img.name}/></button>{/each}</div>{#if !images.length}<p class="p-8 text-slate-400">图库为空，请先在图片管理上传。</p>{/if}</section></div>{/if}
{#if deletion}<div class="fixed inset-0 z-50 grid place-items-center bg-slate-900/40 p-6"><section class="rounded-2xl bg-[#fffdfa]/90 p-6"><h2 class="font-normal">删除「{deletion.title}」？</h2><p class="my-4 text-sm text-rose-700">原文和搜索索引会一起删除。</p><button class={btn} onclick={()=>deletion=null}>取消</button><button class={`${primary} ml-3`} disabled={busy} onclick={()=>run(async()=>{await request(`/documents/${deletion?.id}`,{method:"DELETE"});deletion=null;await load();})}>确认删除</button></section></div>{/if}
