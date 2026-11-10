<script lang="ts">
import { onMount } from "svelte";
import type { Talk, TalkList } from "@/types/talk";
import { apiFetch } from "@/utils/api-client";
import {
	adminButton as button,
	adminField as field,
	adminPrimary as primary,
} from "./ui";

let items: Talk[] = [];
let total = 0;
let page = 1;
let busy = false;
let ready = false;
let error = "";
let message = "";
let draft: Talk | null = null;
let dirty = false;
let imageUrl = "";
let library: { id: number; url: string; name: string }[] = [];
let picking = false;
const card = "rounded-3xl border border-slate-200 bg-[#fffdfa]/90 p-5 md:p-7";
export function canLeave() {
	return !busy && (!dirty || confirm("说说有未保存的修改，确定放弃？"));
}
async function run(action: () => Promise<void>) {
	if (busy) return;
	busy = true;
	error = "";
	message = "";
	try {
		await action();
	} catch (e) {
		error = e instanceof Error ? e.message : "操作失败";
	} finally {
		busy = false;
	}
}
async function load(next = page) {
	const data = await apiFetch<TalkList>(
		`/admin/talks?page=${next}&page_size=12`,
	);
	items = data.items;
	total = data.total;
	page = data.page;
	ready = true;
}
function edit(talk?: Talk) {
	if (!canLeave()) return;
	draft = talk
		? structuredClone(talk)
		: {
				id: 0,
				content: "",
				images: [],
				published: false,
				is_pinned: false,
				revision: 0,
				created_at: "",
				updated_at: "",
				published_at: null,
				likes: 0,
				liked: false,
			};
	dirty = false;
	picking = false;
	imageUrl = "";
	error = "";
	message = "";
}
function close() {
	if (canLeave()) {
		draft = null;
		dirty = false;
		picking = false;
	}
}
async function save() {
	if (!draft) return;
	await run(async () => {
		if (!draft) return;
		draft = await apiFetch<Talk>(
			`/admin/talks${draft.id ? `/${draft.id}` : ""}`,
			{ method: draft.id ? "PUT" : "POST", body: JSON.stringify(draft) },
		);
		dirty = false;
		message = draft.published
			? "说说已发布，前台刷新即可查看"
			: "说说已保存为草稿";
		await load(1);
	});
}
async function change(talk: Talk, patch: Partial<Talk>) {
	if (!canLeave()) return;
	await run(async () => {
		await apiFetch(`/admin/talks/${talk.id}`, {
			method: "PUT",
			body: JSON.stringify({ ...talk, ...patch }),
		});
		draft = null;
		dirty = false;
		await load();
		message = "说说状态已更新";
	});
}
async function remove(talk: Talk) {
	if (
		!canLeave() ||
		!confirm(
			"删除这条说说？删除后前台及其点赞、评论入口将不再展示，已上传的素材文件会保留。",
		)
	)
		return;
	await run(async () => {
		await apiFetch(`/admin/talks/${talk.id}`, { method: "DELETE" });
		draft = null;
		dirty = false;
		await load(items.length === 1 && page > 1 ? page - 1 : page);
		message = "说说已删除";
	});
}
function addImage(url: string) {
	if (!draft) return;
	if (draft.images.length >= 9) throw Error("每条说说最多9张图片");
	if (!draft.images.includes(url)) {
		draft.images = [...draft.images, url];
		dirty = true;
	}
}
async function upload(event: Event) {
	const input = event.currentTarget as HTMLInputElement;
	const files = [...(input.files || [])];
	await run(async () => {
		if (!draft || draft.images.length + files.length > 9)
			throw Error("每条说说最多9张图片");
		for (const file of files) {
			if (file.size > 8 * 1024 * 1024) throw Error(`${file.name} 超过8 MiB`);
			const body = new FormData();
			body.append("file", file);
			const image = await apiFetch<{ url: string }>("/admin/images", {
				method: "POST",
				body,
			});
			addImage(image.url);
		}
		message = "图片已添加，请保存说说";
	});
	input.value = "";
}
function move(index: number, delta: number) {
	if (!draft) return;
	const images = [...draft.images];
	[images[index], images[index + delta]] = [
		images[index + delta],
		images[index],
	];
	draft.images = images;
	dirty = true;
}
function time(value: string) {
	return new Date(value).toLocaleString("zh-CN", {
		timeZone: "Asia/Shanghai",
		hour12: false,
	});
}
onMount(() => {
	void run(() => load());
	const before = (event: BeforeUnloadEvent) => {
		if (dirty || busy) {
			event.preventDefault();
			event.returnValue = "";
		}
	};
	window.addEventListener("beforeunload", before);
	return () => window.removeEventListener("beforeunload", before);
});
</script>
<div class="space-y-6">
 <div class="flex flex-wrap items-center justify-between gap-3"><p class="text-sm text-slate-500">记录日常、想法与此刻的心情。</p><div class="flex gap-2"><button class={button} disabled={busy} onclick={()=>{if(canLeave()){draft=null;dirty=false;void run(()=>load());}}}>刷新</button><button class={primary} disabled={busy||!ready} onclick={()=>edit()}>发布说说</button><a class={button} href="/talks/" target="_blank" rel="noopener noreferrer">查看前台 ↗</a></div></div>
 {#if error}<p role="alert" class="rounded-2xl bg-red-50 p-4 text-sm text-red-700">{error}</p>{/if}
 {#if message}<p role="status" class="rounded-2xl bg-teal-50 p-4 text-sm text-teal-800">{message}</p>{/if}
 {#if draft}<form class={card} oninput={()=>{dirty=true;}} onsubmit={e=>{e.preventDefault();void save();}}>
  <div class="mb-5 flex justify-between gap-3"><h2 class="text-lg text-slate-800">{draft.id?"编辑说说":"写一条说说"}{dirty?" · 未保存":""}</h2><button type="button" class={button} disabled={busy} onclick={close}>关闭编辑</button></div>
  <fieldset disabled={busy} class="space-y-5">
   <label class="block space-y-2 text-sm text-slate-600">说说内容<textarea class={`${field} min-h-44 leading-7`} bind:value={draft.content} maxlength="5000" placeholder="今天有什么想分享的？"></textarea><span class="block text-right text-xs text-slate-400">{draft.content.length} / 5000</span></label>
   <div class="flex flex-wrap gap-2"><label class={`${button} cursor-pointer`}>上传图片<input class="sr-only" type="file" accept="image/jpeg,image/png,image/webp,image/gif,image/avif" multiple onchange={upload}/></label><button type="button" class={button} onclick={()=>run(async()=>{library=await apiFetch<typeof library>("/admin/images");picking=!picking;})}>从素材库选择</button><span class="self-center text-xs text-slate-400">{draft.images.length} / 9 张 · 每张最多8 MiB</span></div>
   {#if picking}<div class="grid max-h-64 grid-cols-3 gap-3 overflow-y-auto rounded-2xl bg-slate-50 p-3 sm:grid-cols-5">{#each library as image}<button type="button" class="overflow-hidden rounded-xl border border-slate-200" disabled={draft.images.includes(image.url)||draft.images.length>=9} onclick={()=>run(async()=>addImage(image.url))}><img src={image.url} alt={image.name} loading="lazy" class="aspect-square w-full object-cover"/></button>{/each}{#if !library.length}<p class="col-span-full py-5 text-center text-sm text-slate-500">素材库暂无图片</p>{/if}</div>{/if}
   <div class="flex gap-2"><input class={field} aria-label="说说图片地址" bind:value={imageUrl} placeholder="HTTPS 或站内图片地址"/><button type="button" class={`${button} shrink-0`} disabled={!imageUrl.trim()} onclick={()=>run(async()=>{addImage(imageUrl.trim());imageUrl="";})}>添加</button></div>
   <div class="grid grid-cols-2 gap-3 sm:grid-cols-3">{#each draft.images as image,index}<div class="space-y-2 rounded-2xl border border-slate-200 p-2"><img src={image} alt={`说说配图 ${index+1}`} class="aspect-square w-full rounded-xl object-cover"/><div class="flex flex-wrap gap-1"><button type="button" class={button} aria-label={`前移图片 ${index+1}`} disabled={index===0} onclick={()=>move(index,-1)}>←</button><button type="button" class={button} aria-label={`后移图片 ${index+1}`} disabled={index===draft!.images.length-1} onclick={()=>move(index,1)}>→</button><button type="button" class={`${button} text-red-600`} onclick={()=>{if(draft){draft.images=draft.images.filter((_,i)=>i!==index);dirty=true;}}}>移除</button></div></div>{/each}</div>
   <div class="flex flex-wrap items-center gap-5 border-t border-slate-100 pt-5"><label class="flex gap-2 text-sm text-slate-600"><input type="checkbox" bind:checked={draft.published}/>发布到前台</label><label class="flex gap-2 text-sm text-slate-600"><input type="checkbox" bind:checked={draft.is_pinned}/>置顶</label><button class={`${primary} ml-auto`}>{busy?"保存中…":"保存说说"}</button></div>
  </fieldset>
 </form>{/if}
 <section class={`${card} space-y-5`}><h2 class="text-lg text-slate-800">全部说说 · {total}</h2>{#if !ready}<p class="py-8 text-sm text-slate-500">{busy?"正在加载…":"加载失败，请刷新重试。"}</p>{:else if !items.length}<p class="py-12 text-center text-sm text-slate-400">还没有说说，记录此刻的心情吧。</p>{:else}{#each items as talk}<article class="space-y-4 rounded-2xl border border-slate-200 p-4"><div class="flex flex-wrap gap-2 text-xs"><span class={`rounded-full px-3 py-1 ${talk.published?"bg-teal-50 text-teal-700":"bg-amber-50 text-amber-700"}`}>{talk.published?"已发布":"草稿 / 已下架"}</span>{#if talk.is_pinned}<span class="rounded-full bg-indigo-50 px-3 py-1 text-indigo-700">置顶</span>{/if}<time class="ml-auto text-slate-400">{time(talk.published_at||talk.created_at)}</time></div><p class="whitespace-pre-wrap break-words text-sm leading-7 text-slate-700">{talk.content}</p>{#if talk.images.length}<div class="flex flex-wrap gap-2">{#each talk.images as image,index}<img src={image} alt={`配图 ${index+1}`} loading="lazy" class="h-20 w-20 rounded-xl object-cover"/>{/each}</div>{/if}<div class="flex flex-wrap gap-2"><button class={button} disabled={busy} onclick={()=>edit(talk)}>编辑</button><button class={button} disabled={busy} onclick={()=>change(talk,{published:!talk.published})}>{talk.published?"下架":"发布"}</button><button class={button} disabled={busy} onclick={()=>change(talk,{is_pinned:!talk.is_pinned})}>{talk.is_pinned?"取消置顶":"置顶"}</button>{#if talk.published}<a class={button} href={`/talks/?id=${talk.id}`} target="_blank" rel="noopener noreferrer">查看</a>{/if}<button class={`${button} text-red-600`} disabled={busy} onclick={()=>remove(talk)}>删除</button></div></article>{/each}{/if}
 {#if total>12}<div class="flex items-center justify-center gap-4"><button class={button} disabled={busy||page===1} onclick={()=>{if(canLeave()){draft=null;dirty=false;void run(()=>load(page-1));}}}>上一页</button><span class="text-sm text-slate-500">{page} / {Math.ceil(total/12)}</span><button class={button} disabled={busy||page*12>=total} onclick={()=>{if(canLeave()){draft=null;dirty=false;void run(()=>load(page+1));}}}>下一页</button></div>{/if}</section>
</div>
