<script lang="ts">
import { onMount } from "svelte";
import { apiFetch } from "@/utils/api-client";
import type { ManagedAlbum } from "@/types/managed-gallery";
import {
	adminButton as button,
	adminPrimary as primary,
	adminField as field,
} from "./ui";
let albums: ManagedAlbum[] = [];
let draft: ManagedAlbum | null = null;
let tags = "";
let dirty = false;
let busy = false;
let loaded = false;
let error = "";
let message = "";
let picking = false;
let library: { id: number; name: string; url: string }[] = [];
let selected: number[] = [];
let photoUrl = "";
let photoCaption = "";
const card = "rounded-3xl border border-slate-200 bg-[#fffdfa]/90 p-5 md:p-7";
export function canLeave() {
	return !busy && (!dirty || confirm("相册有未保存的修改，确定放弃？"));
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
async function load() {
	albums = await apiFetch<ManagedAlbum[]>("/admin/gallery/albums");
	loaded = true;
}
function edit(album?: ManagedAlbum) {
	if (!canLeave()) return;
	draft = album
		? structuredClone(album)
		: {
				id: 0,
				slug: `album-${Date.now()}`,
				name: "",
				description: "",
				date: "",
				location: "",
				cover: "",
				tags: [],
				photos: [],
				published: false,
				sort_order: albums.length,
			};
	tags = draft.tags.join(", ");
	dirty = false;
	picking = false;
	photoUrl = "";
	photoCaption = "";
	error = "";
	message = "";
}
function close() {
	if (!canLeave()) return;
	draft = null;
	dirty = false;
	picking = false;
}
async function save() {
	if (!draft) return;
	await run(async () => {
		if (!draft) return;
		draft.tags = [
			...new Set(
				tags
					.split(/[,，]/)
					.map((t) => t.trim())
					.filter(Boolean),
			),
		];
		const saved = await apiFetch<ManagedAlbum>(
			`/admin/gallery/albums${draft.id ? `/${draft.id}` : ""}`,
			{
				method: draft.id ? "PUT" : "POST",
				body: JSON.stringify(draft),
			},
		);
		draft = saved;
		dirty = false;
		message = saved.published
			? "相册已发布，前台刷新即可查看"
			: "相册已保存为草稿";
		await load();
	});
}
async function remove(album: ManagedAlbum) {
	if (
		!canLeave() ||
		!confirm(`删除相册“${album.name}”及其中的照片关联？图片素材文件会保留。`)
	)
		return;
	await run(async () => {
		await apiFetch(`/admin/gallery/albums/${album.id}`, { method: "DELETE" });
		if (draft?.id === album.id) {
			draft = null;
			dirty = false;
		}
		await load();
		message = "相册已删除，图片文件仍保留在素材库";
	});
}
function addPhoto(url: string, caption = "") {
	if (!draft) return;
	if (draft.photos.length >= 500) throw new Error("每个相册最多500张照片");
	if (draft.photos.some((p) => p.url === url)) return;
	draft.photos = [...draft.photos, { id: crypto.randomUUID(), url, caption }];
	draft = { ...draft };
	dirty = true;
}
async function upload(event: Event) {
	const input = event.currentTarget as HTMLInputElement;
	const files = [...(input.files || [])];
	await run(async () => {
		if (!draft || draft.photos.length + files.length > 500)
			throw new Error("每个相册最多500张照片");
		for (const file of files) {
			if (file.size > 8 * 1024 * 1024)
				throw new Error(`${file.name} 超过8 MiB`);
			const body = new FormData();
			body.append("file", file);
			const image = await apiFetch<{ url: string }>("/admin/images", {
				method: "POST",
				body,
			});
			addPhoto(image.url, file.name.replace(/\.[^.]+$/, ""));
		}
		message = "照片已添加，请保存相册";
	});
	input.value = "";
}
async function openLibrary() {
	await run(async () => {
		library = await apiFetch<typeof library>("/admin/images");
		selected = [];
		picking = true;
	});
}
function movePhoto(index: number, delta: number) {
	if (!draft || index + delta < 0 || index + delta >= draft.photos.length)
		return;
	const photos = [...draft.photos];
	[photos[index], photos[index + delta]] = [
		photos[index + delta],
		photos[index],
	];
	draft = { ...draft, photos };
	dirty = true;
}
onMount(() => {
	void run(load);
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
 <div class="flex flex-wrap items-center justify-between gap-3"><p class="text-sm text-slate-500">管理相册、封面与照片，发布后即可在前台查看。</p><div class="flex gap-2"><button class={button} disabled={busy} onclick={() => { if(canLeave()) { draft=null;dirty=false;void run(load); } }}>刷新列表</button><button class={primary} disabled={busy || !loaded} onclick={() => edit()}>新建相册</button><a class={button} href="/gallery/" target="_blank" rel="noopener noreferrer">查看相册 ↗</a></div></div>
 {#if error}<p role="alert" class="rounded-2xl bg-red-50 p-4 text-sm text-red-700">{error}</p>{/if}
 {#if message}<p role="status" class="rounded-2xl bg-teal-50 p-4 text-sm text-teal-800">{message}</p>{/if}
 {#if draft}
 <form class={card} oninput={() => { dirty = true; }} onsubmit={e => {e.preventDefault();void save();}}>
  <div class="mb-6 flex flex-wrap items-center justify-between gap-3"><h2 class="text-lg text-slate-800">{draft.id ? "编辑相册" : "新建相册"}{dirty ? " · 未保存" : ""}</h2><button type="button" class={button} disabled={busy} onclick={close}>关闭编辑</button></div>
  <fieldset disabled={busy} class="space-y-5">
   <div class="grid gap-5 sm:grid-cols-2">
    <label class="space-y-2 text-sm text-slate-600">相册名称<input class={field} bind:value={draft.name} required maxlength="200" /></label>
    <label class="space-y-2 text-sm text-slate-600">相册标识<input class={field} bind:value={draft.slug} required pattern="[a-z0-9-]+" maxlength="120" /><span class="block text-xs text-slate-400">用于分享链接，只能包含小写字母、数字和短横线。</span></label>
    <label class="space-y-2 text-sm text-slate-600">拍摄日期<input class={field} type="date" bind:value={draft.date} /></label>
    <label class="space-y-2 text-sm text-slate-600">拍摄地点<input class={field} bind:value={draft.location} maxlength="200" /></label>
    <label class="space-y-2 text-sm text-slate-600 sm:col-span-2">相册描述<textarea class={`${field} min-h-24`} bind:value={draft.description} maxlength="5000"></textarea></label>
    <label class="space-y-2 text-sm text-slate-600">标签<input class={field} bind:value={tags} placeholder="风景, 旅行" /></label>
    <label class="space-y-2 text-sm text-slate-600">显示顺序<input class={field} type="number" min="0" max="1000000" step="1" bind:value={draft.sort_order} required /><span class="block text-xs text-slate-400">数字越小越靠前。</span></label>
    <label class="space-y-2 text-sm text-slate-600 sm:col-span-2">封面地址<input class={field} bind:value={draft.cover} placeholder="留空使用第一张照片，也可点击照片下的设为封面" /></label>
   </div>
   {#if draft.cover || draft.photos[0]}<img src={draft.cover || draft.photos[0].url} alt="相册封面预览" class="h-40 w-full rounded-2xl object-cover sm:w-64" />{/if}
   <div class="flex flex-wrap items-center justify-between gap-3 border-t border-slate-100 pt-5"><h3 class="text-base text-slate-700">照片 · {draft.photos.length} / 500</h3><div class="flex flex-wrap gap-2"><label class={`${button} cursor-pointer`}>上传照片<input type="file" class="sr-only" multiple accept="image/jpeg,image/png,image/webp,image/gif,image/avif" onchange={upload} /></label><button type="button" class={button} onclick={openLibrary}>从素材库选择</button></div></div>
   <p class="text-xs text-slate-400">每张最多8 MiB；上传后请保存相册。移除照片只解除关联。</p>
   {#if picking}
    <div class="space-y-3 rounded-2xl bg-slate-50 p-4"><div class="flex items-center justify-between"><h4 class="text-sm text-slate-600">选择已有图片</h4><button type="button" class={button} onclick={()=>{picking=false;}}>取消</button></div><div class="grid max-h-72 grid-cols-3 gap-3 overflow-y-auto sm:grid-cols-5">{#each library as image}<label class="relative overflow-hidden rounded-xl border border-slate-200 bg-white p-1"><img src={image.url} alt={image.name} loading="lazy" class="aspect-square w-full object-cover" /><span class="mt-1 flex gap-1 text-xs"><input type="checkbox" bind:group={selected} value={image.id} />{image.name}</span></label>{/each}</div>{#if !library.length}<p class="text-sm text-slate-500">素材库暂无图片，请先上传。</p>{/if}<button type="button" class={primary} disabled={!selected.length} onclick={()=>run(async()=>{for(const image of library.filter(i=>selected.includes(i.id)))addPhoto(image.url);picking=false;})}>添加选中的 {selected.length} 张图片</button></div>
   {/if}
   <div class="grid gap-3 sm:grid-cols-[1fr_1fr_auto]"><input class={field} aria-label="照片地址" bind:value={photoUrl} placeholder="HTTPS 或站内图片地址" /><input class={field} aria-label="照片说明" bind:value={photoCaption} placeholder="照片说明（可选）" /><button type="button" class={button} disabled={!photoUrl.trim()} onclick={()=>run(async()=>{addPhoto(photoUrl.trim(),photoCaption.trim());photoUrl="";photoCaption="";})}>添加链接</button></div>
   <div class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">{#each draft.photos as photo,index (photo.id)}<div class="space-y-3 rounded-2xl border border-slate-200 p-3"><img src={photo.url} alt={photo.caption || `照片 ${index+1}`} loading="lazy" class="aspect-[4/3] w-full rounded-xl object-cover" /><label class="block text-xs text-slate-500">照片说明<input class={`${field} mt-2`} bind:value={photo.caption} maxlength="1000" /></label><div class="flex flex-wrap gap-2"><button type="button" class={button} disabled={index===0} aria-label={`上移照片 ${index+1}`} onclick={()=>movePhoto(index,-1)}>↑</button><button type="button" class={button} disabled={index===draft!.photos.length-1} aria-label={`下移照片 ${index+1}`} onclick={()=>movePhoto(index,1)}>↓</button><button type="button" class={button} onclick={()=>{if(draft){draft.cover=photo.url;dirty=true;}}}>{draft.cover===photo.url ? "当前封面" : "设为封面"}</button><button type="button" class={`${button} text-red-600`} onclick={()=>{if(draft){draft.photos=draft.photos.filter(p=>p.id!==photo.id);if(draft.cover===photo.url)draft.cover="";dirty=true;}}}>移除</button></div></div>{/each}</div>
   {#if !draft.photos.length}<p class="py-8 text-center text-sm text-slate-400">添加照片，开始记录这一段风景。</p>{/if}
   <div class="flex flex-wrap items-center justify-between gap-4 border-t border-slate-100 pt-5"><label class="flex items-center gap-2 text-sm text-slate-600"><input type="checkbox" bind:checked={draft.published} />发布到前台（未勾选则保存为草稿）</label><button class={primary}>{busy ? "保存中…" : "保存相册"}</button></div>
  </fieldset>
 </form>
 {/if}
 <section class={card}><h2 class="mb-5 text-lg text-slate-800">全部相册 · {albums.length}</h2>{#if !loaded}<p class="py-8 text-sm text-slate-500">{busy ? "正在加载相册…" : "相册加载失败，请重试。"}</p>{:else if !albums.length}<p class="py-10 text-center text-sm text-slate-400">还没有相册，点击“新建相册”添加。</p>{:else}<div class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">{#each albums as album}<article class="overflow-hidden rounded-2xl border border-slate-200">{#if album.cover || album.photos[0]}<img src={album.cover || album.photos[0].url} alt={album.name} class="aspect-video w-full object-cover" loading="lazy" />{:else}<div class="grid aspect-video place-items-center bg-slate-50 text-slate-400">暂无照片</div>{/if}<div class="space-y-3 p-4"><div class="flex items-center justify-between gap-2"><h3 class="truncate text-base text-slate-700">{album.name}</h3><span class={`shrink-0 rounded-full px-2 py-1 text-xs ${album.published ? "bg-teal-50 text-teal-700" : "bg-amber-50 text-amber-700"}`}>{album.published ? "已发布" : "草稿"}</span></div><p class="text-xs text-slate-500">{album.photos.length} 张照片 · 顺序 {album.sort_order}{album.location ? ` · ${album.location}` : ""}</p><div class="flex flex-wrap gap-2"><button class={button} disabled={busy} onclick={()=>edit(album)}>编辑</button>{#if album.published}<a class={button} href={`/gallery/?album=${encodeURIComponent(album.slug)}`} target="_blank" rel="noopener noreferrer">查看</a>{/if}<button class={`${button} text-red-600`} disabled={busy} onclick={()=>remove(album)}>删除</button></div></div></article>{/each}</div>{/if}</section>
</div>
