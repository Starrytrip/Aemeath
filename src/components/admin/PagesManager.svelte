<script lang="ts">
import { onMount } from "svelte";
import MarkdownEditor from "./MarkdownEditor.svelte";
import { apiFetch } from "@/utils/api-client";
import type { ManagedPageData } from "@/types/managed-site";
import { adminButton as button, adminPrimary, adminField as field } from "./ui";
let slug = "about";
let current = "about";
let revision = 0;
let busy = true;
let dirty = false;
let error = "";
let message = "";
let form: ManagedPageData = { title: "关于我", markdown: "", published: false };
let preview = "";
let editor: MarkdownEditor;
let editorMode: "visual" | "source" = "visual";
let editorVersion = 0;
let libraryOpen = false;
let images: { id: number; url: string; name: string }[] = [];
function changeMarkdown(markdown: string) {
	if (markdown === form.markdown) return;
	form.markdown = markdown;
	dirty = true;
	preview = "";
	message = "";
}
function flush() {
	if (editorMode === "visual" && editor) changeMarkdown(editor.getValue());
}
function switchMode(mode: typeof editorMode) {
	flush();
	editorMode = mode;
}
async function library() {
	try {
		images = await apiFetch("/admin/images");
		libraryOpen = true;
	} catch (e) {
		error = e instanceof Error ? e.message : "图库加载失败";
	}
}
function insertImage(url: string) {
	const markdown = `![](${url})\n`;
	if (editorMode === "visual") editor?.insert(markdown);
	else changeMarkdown(`${form.markdown}\n${markdown}`);
	libraryOpen = false;
}

const pages = [
	{ slug: "about", title: "关于我", url: "/about/" },
	{
		slug: "about-open-source",
		title: "开源项目介绍",
		url: "/page/about-open-source/",
	},
	{ slug: "guestbook", title: "留言页说明", url: "/guestbook/" },
	{ slug: "friends", title: "友链页说明", url: "/page/friends/" },
];
export function canLeave() {
	flush();
	return !dirty || confirm("页面有未保存的修改，确定离开？");
}
async function load(next = slug) {
	if (!canLeave()) {
		slug = current;
		return;
	}
	busy = true;
	error = "";
	preview = "";
	try {
		const r = await apiFetch<{
			data: ManagedPageData | null;
			revision: number;
		}>(`/admin/site/page-${next}`);
		form = r.data || {
			title: pages.find((p) => p.slug === next)?.title || next,
			markdown: "",
			published: false,
		};
		current = next;
		slug = next;
		revision = r.revision;
		dirty = false;
		editorVersion++;
		message = "";
	} catch (e) {
		error = e instanceof Error ? e.message : "加载失败";
	} finally {
		busy = false;
	}
}
async function save() {
	flush();
	busy = true;
	error = "";
	try {
		const r = await apiFetch<{ revision: number }>(
			`/admin/site/page-${current}`,
			{ method: "PUT", body: JSON.stringify({ data: form, revision }) },
		);
		revision = r.revision;
		dirty = false;
		message = form.published ? "页面已发布" : "已保存为草稿，前台不显示正文";
	} catch (e) {
		error = e instanceof Error ? e.message : "保存失败";
	} finally {
		busy = false;
	}
}
async function showPreview() {
	flush();
	try {
		preview = (
			await apiFetch<{ html: string }>("/admin/page-preview", {
				method: "POST",
				body: JSON.stringify({ markdown: form.markdown }),
			})
		).html;
	} catch (e) {
		error = e instanceof Error ? e.message : "预览失败";
	}
}
onMount(() => {
	void load();
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
<div class="mb-5 flex flex-wrap items-center justify-between gap-3"><label>选择页面 <select class="ml-2 rounded-xl border border-slate-200 bg-white p-2" bind:value={slug} onchange={()=>load(slug)} disabled={busy}>{#each pages as p}<option value={p.slug}>{p.title}</option>{/each}</select></label><div class="flex gap-2"><button class={button} disabled={busy} onclick={()=>load()}>重新加载</button><button class={button} disabled={busy} onclick={showPreview}>预览</button><button class={adminPrimary} disabled={busy} onclick={save}>保存页面</button></div></div>
{#if error}<p role="alert" class="mb-4 text-red-600">{error}</p>{/if}{#if message}<p role="status" class="mb-4 text-emerald-700">{message}</p>{/if}
<fieldset disabled={busy} class="space-y-5 rounded-3xl border border-slate-200 bg-white/90 p-6" oninput={()=>{dirty=true;preview="";message="";}}><label class="block text-sm">页面标题<input class={`${field} mt-2`} bind:value={form.title}/></label><label><input type="checkbox" bind:checked={form.published}/> 发布页面正文</label><div class="space-y-3"><div class="flex flex-wrap items-center justify-between gap-3"><span class="text-sm">页面正文</span><div class="flex gap-2" role="group" aria-label="正文编辑方式"><button type="button" class={editorMode==="visual"?adminPrimary:button} aria-pressed={editorMode==="visual"} onclick={()=>switchMode("visual")}>可视化编辑</button><button type="button" class={editorMode==="source"?adminPrimary:button} aria-pressed={editorMode==="source"} onclick={()=>switchMode("source")}>Markdown 源码</button></div></div>
{#if editorMode==="visual"}{#if !busy}{#key `${current}:${editorVersion}`}<MarkdownEditor bind:this={editor} value={form.markdown} mode="wysiwyg" onchange={changeMarkdown} onlibrary={library}/>{/key}{:else}<p class="p-6 text-sm text-slate-500" role="status">正在处理页面内容…</p>{/if}{:else}<label class="block text-sm">Markdown 源码<textarea class={`${field} mt-2 min-h-[28rem] resize-y font-mono leading-7`} value={form.markdown} oninput={e=>changeMarkdown(e.currentTarget.value)} spellcheck="false"></textarea></label>{/if}</div><p class="text-sm text-slate-500">只需编辑文字和图片，页面样式由模板统一处理。头像、昵称和社交链接请到“站点设置 → 博主资料”修改。{dirty?"当前有未保存的修改。":""}</p></fieldset>
{#if preview}<section class="prose mt-5 max-w-none rounded-3xl border bg-white p-6"><h2>{form.title}</h2>{@html preview}</section>{/if}

{#if libraryOpen}<div class="fixed inset-0 z-50 grid place-items-center bg-black/40 p-5"><div role="dialog" aria-modal="true" aria-label="插入图片" tabindex="-1" class="max-h-[80vh] w-full max-w-3xl overflow-y-auto rounded-2xl bg-white p-5"><div class="mb-4 flex justify-between"><h2>插入图片</h2><button type="button" class={button} onclick={()=>libraryOpen=false}>关闭</button></div><div class="grid grid-cols-3 gap-3 sm:grid-cols-5">{#each images as image}<button type="button" class="rounded-xl border p-2 hover:border-cyan-500" onclick={()=>insertImage(image.url)}><img src={image.url} alt={image.name} class="aspect-square w-full object-contain" loading="lazy"/><span class="block truncate text-xs">{image.name}</span></button>{/each}</div>{#if !images.length}<p class="p-8 text-center">图库还没有图片，可使用编辑器工具栏上传。</p>{/if}</div></div>{/if}
