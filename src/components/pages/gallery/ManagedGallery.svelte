<script lang="ts">
import { onMount } from "svelte";
import type { ManagedAlbum } from "@/types/managed-gallery";

let albums: ManagedAlbum[] = [];
let loading = true;
let error = "";
let query = "";
let tag = "";
let slug = "";
let photoIndex = 0;
let viewer: HTMLDialogElement;
$: album = albums.find((a) => a.slug === slug);
$: tags = [...new Set(albums.flatMap((a) => a.tags))].sort();
$: filtered = albums.filter(
	(a) =>
		(!tag || a.tags.includes(tag)) &&
		`${a.name} ${a.description} ${a.location} ${a.tags.join(" ")}`
			.toLowerCase()
			.includes(query.trim().toLowerCase()),
);
$: photo = album?.photos[photoIndex];
const control =
	"rounded-xl border border-(--line-divider) px-4 py-2 text-sm transition-colors hover:bg-(--btn-regular-bg-hover) focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-(--primary)";
async function load() {
	loading = true;
	error = "";
	try {
		const response = await fetch("/api/v1/gallery/albums", {
			signal: AbortSignal.timeout(10000),
		});
		if (!response.ok) throw new Error("相册暂时无法加载，请稍后重试。");
		const rows = await response.json();
		if (!Array.isArray(rows)) throw new Error("相册数据格式不正确。");
		albums = rows;
	} catch (e) {
		error = e instanceof Error ? e.message : "相册加载失败";
	} finally {
		loading = false;
	}
}
function navigate(next: string) {
	viewer?.close();
	slug = next;
	photoIndex = 0;
	const target = new URL(location.href);
	if (next) target.searchParams.set("album", next);
	else target.searchParams.delete("album");
	history.pushState({}, "", target);
}
function showPhoto(index: number) {
	photoIndex = index;
	viewer.showModal();
}
onMount(() => {
	slug = new URL(location.href).searchParams.get("album") || "";
	void load();
	const pop = () => {
		viewer?.close();
		slug = new URL(location.href).searchParams.get("album") || "";
		photoIndex = 0;
	};
	window.addEventListener("popstate", pop);
	return () => {
		window.removeEventListener("popstate", pop);
		viewer?.close();
	};
});
</script>

<section class="gallery-surface card-base w-full rounded-(--radius-large) p-4 text-neutral-800 dark:text-neutral-100 sm:p-7">
 {#if loading}<p role="status" class="py-16 text-center text-neutral-500">正在加载相册…</p>
 {:else if error}<div role="alert" class="space-y-4 py-12 text-center"><p>{error}</p><button class={control} onclick={load}>重新加载</button></div>
 {:else if slug && !album}<div class="space-y-4 py-16 text-center"><h1 class="text-2xl">相册不存在或尚未发布</h1><button class={control} onclick={()=>navigate("")}>返回全部相册</button></div>
 {:else if album}
  <button class={`${control} mb-5`} onclick={()=>navigate("")}>← 全部相册</button>
  <div class="album-heading">
   {#if album.cover || album.photos[0]}<div class="album-heading__cover"><img src={album.cover || album.photos[0].url} alt="" class="h-full w-full object-cover" fetchpriority="high" /></div>{/if}
   <div><p class="mb-2 text-xs tracking-widest text-neutral-500">PHOTO ALBUM</p><h1 class="text-3xl font-bold">{album.name}</h1></div>
  </div>
  {#if album.description}<p class="mt-4 whitespace-pre-line text-sm leading-7 text-neutral-600 dark:text-neutral-300">{album.description}</p>{/if}
  <p class="mt-4 text-sm text-neutral-500">{album.date}{album.location ? ` · ${album.location}` : ""} · {album.photos.length} 张照片</p>
  <div class="mt-3 flex flex-wrap gap-2">{#each album.tags as item}<span class="rounded-full bg-(--btn-regular-bg) px-3 py-1 text-xs text-(--primary)">{item}</span>{/each}</div>
  {#if !album.photos.length}<p class="py-16 text-center text-neutral-400">这个相册还没有照片。</p>{:else}
  <div class="photo-masonry">{#each album.photos as image,index (image.id)}
   <figure class="photo-pin">
    <button class="photo-pin__image" aria-label={`查看大图：${image.caption || `照片 ${index+1}`}`} onclick={()=>showPhoto(index)}>
     <img src={image.url} alt={image.caption || `${album!.name} · 照片 ${index+1}`} loading="lazy" decoding="async" />
     <span class="photo-pin__expand" aria-hidden="true"><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><path d="M8 3H3v5m13-5h5v5M3 16v5h5m13-5v5h-5" /></svg></span>
    </button>
    {#if image.caption}<figcaption>{image.caption}</figcaption>{/if}
   </figure>
  {/each}</div>
  {/if}
 {:else}
  <h1 class="text-3xl font-bold">相册</h1><p class="mt-3 text-sm text-neutral-500">收藏沿途风景，记录生活里的片刻。</p>
  {#if albums.length}
   <label class="mt-6 block"><span class="sr-only">搜索相册</span><input class="w-full rounded-xl border border-(--line-divider) bg-transparent px-4 py-3 text-sm outline-none focus:border-(--primary)" bind:value={query} placeholder="搜索相册、地点或标签…" /></label>
   {#if tags.length}<div class="my-4 flex flex-wrap gap-2"><button class={control} aria-pressed={!tag} onclick={()=>{tag="";}}>全部</button>{#each tags as item}<button class={control} aria-pressed={tag===item} onclick={()=>{tag=item;}}>{item}</button>{/each}</div>{/if}
   <div class="album-shelf">{#each filtered as item}<button class="album-frame" onclick={()=>navigate(item.slug)}><div class="album-frame__cover">{#if item.cover || item.photos[0]}<img src={item.cover || item.photos[0].url} alt={item.name} loading="lazy" class="aspect-[4/3] w-full object-cover" />{:else}<div class="grid aspect-[4/3] place-items-center bg-(--btn-regular-bg) text-neutral-400">暂无照片</div>{/if}<span class="album-frame__count">{item.photos.length} 张照片</span></div><div class="space-y-3 px-2 pb-3 pt-4"><h2 class="text-lg font-bold">{item.name}</h2>{#if item.description}<p class="line-clamp-2 text-sm leading-6 text-neutral-500">{item.description}</p>{/if}<p class="text-xs text-neutral-500">{item.photos.length} 张照片{item.location ? ` · ${item.location}` : ""}{item.date ? ` · ${item.date}` : ""}</p></div></button>{/each}</div>
   {#if !filtered.length}<p class="py-16 text-center text-neutral-400">没有找到匹配的相册。</p>{/if}
  {:else}<p class="py-16 text-center text-neutral-400">还没有发布相册，敬请期待。</p>{/if}
 {/if}
</section>
<dialog bind:this={viewer} aria-label="照片大图预览" class="max-h-[95dvh] w-[min(96vw,1200px)] max-w-none rounded-2xl bg-neutral-950 p-4 text-white shadow-2xl backdrop:bg-black/80">
 <div class="mb-3 flex items-center justify-between gap-3"><p class="text-sm">{photoIndex+1} / {album?.photos.length || 0}</p><button class="rounded-lg px-4 py-2 hover:bg-white/15" onclick={()=>viewer.close()}>关闭 ✕</button></div>
 {#if photo}<img src={photo.url} alt={photo.caption || `照片 ${photoIndex+1}`} class="mx-auto max-h-[72dvh] max-w-full object-contain" />{#if photo.caption}<p class="mt-3 text-center text-sm">{photo.caption}</p>{/if}{/if}
 <div class="mt-3 flex justify-between"><button class="rounded-lg px-4 py-2 hover:bg-white/15 disabled:opacity-30" disabled={photoIndex===0} onclick={()=>{photoIndex--;}}>← 上一张</button><button class="rounded-lg px-4 py-2 hover:bg-white/15 disabled:opacity-30" disabled={!album || photoIndex>=album.photos.length-1} onclick={()=>{photoIndex++;}}>下一张 →</button></div>
</dialog>


<style>
 .gallery-surface {
  container-type: inline-size;
 }
 .album-heading {
  display: flex;
  align-items: center;
  gap: 1.25rem;
 }
 .album-heading__cover {
  width: 5rem;
  height: 5rem;
  flex-shrink: 0;
  overflow: hidden;
  padding: 4px;
  border: 1px solid var(--line-divider);
  border-radius: 1rem;
  background: var(--card-bg, #fff);
  box-shadow: 3px 3px 0 color-mix(in srgb, var(--primary) 12%, transparent);
 }
 .album-heading__cover img { border-radius: 0.7rem; }
 .photo-masonry {
  columns: 2;
  column-gap: 0.85rem;
  margin-top: 1.75rem;
 }
 .photo-pin {
  display: inline-block;
  width: 100%;
  margin: 0 0 1.15rem;
  vertical-align: top;
  break-inside: avoid;
 }
 .photo-pin__image {
  position: relative;
  display: block;
  width: 100%;
  overflow: hidden;
  border-radius: 1.15rem;
  background: var(--btn-regular-bg);
  cursor: zoom-in;
  isolation: isolate;
 }
 .photo-pin__image img {
  display: block;
  width: 100%;
  height: auto;
  transition: filter 200ms ease;
 }
 .photo-pin__image:focus-visible,
 .album-frame:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 4px;
 }
 .photo-pin__expand {
  position: absolute;
  right: 0.65rem;
  bottom: 0.65rem;
  display: grid;
  width: 2rem;
  height: 2rem;
  place-items: center;
  border-radius: 50%;
  color: white;
  background: rgb(0 0 0 / 40%);
  opacity: 0;
  transition: opacity 200ms ease;
 }
 .photo-pin__image:focus-visible .photo-pin__expand { opacity: 1; }
 .photo-pin figcaption {
  padding: 0.6rem 0.25rem 0;
  font-size: 0.8rem;
  line-height: 1.6;
  overflow-wrap: anywhere;
 }
 .album-shelf {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(100%, 220px), 1fr));
  gap: 1.4rem;
  margin-top: 1.5rem;
  padding: 0 4px 4px 0;
 }
 .album-frame {
  position: relative;
  min-width: 0;
  padding: 0.65rem 0.65rem 0.3rem 1.05rem;
  border: 1px solid color-mix(in srgb, var(--primary) 24%, var(--line-divider));
  border-radius: 1.15rem;
  background: var(--card-bg, #fff);
  box-shadow: 0 4px 14px rgb(0 0 0 / 4%), 3px 3px 0 var(--card-bg, #fff), 4px 4px 0 color-mix(in srgb, var(--primary) 20%, var(--line-divider));
  text-align: left;
  transition: transform 200ms ease, box-shadow 200ms ease;
 }
 .album-frame::before {
  content: "";
  position: absolute;
  top: 0.9rem;
  bottom: 0.9rem;
  left: 0.45rem;
  width: 2px;
  border-radius: 2px;
  background: color-mix(in srgb, var(--primary) 24%, transparent);
 }
 .album-frame__cover {
  position: relative;
  overflow: hidden;
  border: 1px solid var(--line-divider);
  border-radius: 0.65rem;
 }
 .album-frame__count {
  position: absolute;
  right: 0.65rem;
  bottom: 0.65rem;
  padding: 0.3rem 0.65rem;
  border: 1px solid rgb(255 255 255 / 25%);
  border-radius: 999px;
  background: rgb(0 0 0 / 45%);
  color: white;
  font-size: 0.7rem;
 }
 @container (min-width: 580px) {
  .photo-masonry { columns: 3; column-gap: 1rem; }
 }
 @container (min-width: 850px) {
  .photo-masonry { columns: 4; }
 }
 @container (min-width: 1120px) {
  .photo-masonry { columns: 5; }
 }
 @media (hover: hover) {
  .photo-pin__image:hover img { filter: brightness(0.9); }
  .photo-pin__image:hover .photo-pin__expand { opacity: 1; }
  .album-frame:hover {
   transform: translateY(-3px);
   box-shadow: 0 9px 22px rgb(0 0 0 / 7%), 3px 3px 0 var(--card-bg, #fff), 4px 4px 0 color-mix(in srgb, var(--primary) 30%, var(--line-divider));
  }
 }
 @media (prefers-reduced-motion: reduce) {
  .photo-pin__image img, .photo-pin__expand, .album-frame { transition: none; }
  .album-frame:hover { transform: none; }
 }
</style>
