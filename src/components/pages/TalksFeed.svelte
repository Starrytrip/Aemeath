<script lang="ts">
import { onMount } from "svelte";
import type { Talk, TalkList } from "@/types/talk";
import { apiFetch, type PostStats } from "@/utils/api-client";
import TalkComments from "./TalkComments.svelte";
export let author: string;
export let avatar: string;
let items: Talk[] = [];
let total = 0;
let page = 0;
let selected = "";
let busy = false;
let ready = false;
let error = "";
let actionError = "";
let likeBusy: number[] = [];
let opened: number | null = null;
let viewer: HTMLDialogElement;
let activeImage = "";
let activeAlt = "";
async function load(reset = false) {
	if (busy) return;
	busy = true;
	error = "";
	try {
		const next = reset ? 1 : page + 1;
		const data = await apiFetch<TalkList>(
			`/talks?page=${next}&page_size=12${selected ? `&id=${encodeURIComponent(selected)}` : ""}`,
		);
		items = reset
			? data.items
			: [
					...items,
					...data.items.filter(
						(talk) => !items.some((old) => old.id === talk.id),
					),
				];
		total = data.total;
		page = data.page;
		ready = true;
	} catch (e) {
		error = e instanceof Error ? e.message : "说说加载失败，请稍后重试";
	} finally {
		busy = false;
	}
}
async function like(talk: Talk) {
	if (likeBusy.includes(talk.id)) return;
	likeBusy = [...likeBusy, talk.id];
	actionError = "";
	try {
		const stats = await apiFetch<PostStats>("/stats/like", {
			method: "POST",
			body: JSON.stringify({ page: `/talks/${talk.id}/` }),
		});
		items = items.map((item) =>
			item.id === talk.id
				? { ...item, likes: stats.likes, liked: stats.liked }
				: item,
		);
	} catch (e) {
		actionError = e instanceof Error ? e.message : "点赞失败，请重试";
	} finally {
		likeBusy = likeBusy.filter((id) => id !== talk.id);
	}
}
function time(value: string) {
	return new Date(value).toLocaleString("zh-CN", {
		timeZone: "Asia/Shanghai",
		hour12: false,
		year: "numeric",
		month: "2-digit",
		day: "2-digit",
		hour: "2-digit",
		minute: "2-digit",
	});
}
function preview(url: string, index: number) {
	activeImage = url;
	activeAlt = `说说配图 ${index + 1}`;
	viewer.showModal();
}
onMount(() => {
	selected = new URL(location.href).searchParams.get("id") || "";
	void load(true);
	return () => viewer?.close();
});
</script>
<section class="talks-feed" aria-label="说说动态">
 <header class="talks-hero card-base"><span class="talks-eyebrow">LITTLE MOMENTS</span><div class="talks-title"><h1>说说</h1><svg aria-hidden="true" width="30" height="30" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2"><path d="M5 4h14a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H9l-5 3V6a2 2 0 0 1 1-2ZM8 9h8M8 13h5"/></svg></div><p>把日子写成小小的片段，收藏此刻的心情。</p><div class="talks-hero-bottom"><span>日常 · 灵感 · 碎碎念</span>{#if ready}<span>{total} 条{selected?"记录":"说说"}</span>{/if}</div></header>
 {#if selected}<a class="talks-back" href="/talks/">← 查看全部说说</a>{/if}
 {#if error}<div role="alert" class="talks-message card-base"><p>{error}</p><button onclick={()=>load(!ready)}>重新加载</button></div>{/if}
 {#if !ready&&busy}<p role="status" class="talks-message card-base">正在翻开日常的记录…</p>{:else if ready&&!items.length}<div class="talks-empty card-base"><span aria-hidden="true">✦</span><h2>{selected?"这条说说暂不可见":"还没有说说"}</h2><p>{selected?"可能尚未发布、已下架或已删除。":"下一段日常，正在慢慢发生。"}</p></div>{/if}
 {#if actionError}<p role="alert" class="talks-action-error">{actionError}</p>{/if}
 <div class="talks-timeline">{#each items as talk (talk.id)}
  <article class="talk-card card-base" id={`talk-${talk.id}`}>
   <header class="talk-card-header"><img src={avatar} alt="" class="talk-avatar" loading="lazy"/><div class="talk-author"><strong>{author}</strong><time datetime={talk.published_at||talk.created_at}>{time(talk.published_at||talk.created_at)}</time></div>{#if talk.is_pinned}<span class="talk-pinned"><svg aria-hidden="true" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="m8 3 8 0-1 7 4 4H5l4-4-1-7Zm4 11v7"/></svg>置顶</span>{/if}</header>
   {#if talk.content}<p class="talk-content">{talk.content}</p>{/if}
   {#if talk.images.length}<div class="talk-images" class:single={talk.images.length===1} class:two-columns={talk.images.length>=2&&talk.images.length<=4}>{#each talk.images as image,index}<button class="talk-image" aria-label={`查看说说 ${talk.id} 的配图 ${index+1}`} onclick={()=>preview(image,index)}><img src={image} alt={`配图 ${index+1}`} loading="lazy" decoding="async"/></button>{/each}</div>{/if}
   <footer class="talk-card-footer"><button class="talk-action" class:liked={talk.liked} disabled={likeBusy.includes(talk.id)} aria-pressed={talk.liked} aria-label={`${talk.liked?"取消点赞":"点赞"}说说 ${talk.id}`} onclick={()=>like(talk)}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 24 24" fill={talk.liked?"currentColor":"none"} stroke="currentColor" stroke-width="1.5"><path d="M20.8 4.6a5.5 5.5 0 0 0-7.8 0L12 5.7l-1.1-1.1a5.5 5.5 0 0 0-7.8 7.8L12 21l8.8-8.6a5.5 5.5 0 0 0 0-7.8Z"/></svg><span>{talk.likes||"喜欢"}</span></button><button class="talk-action" aria-expanded={opened===talk.id} aria-controls={`talk-comments-${talk.id}`} onclick={()=>{opened=opened===talk.id?null:talk.id;}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M4 4h16v13H9l-5 4V4Z"/></svg>评论</button><a class="talk-permalink" href={`/talks/?id=${talk.id}`} aria-label={`说说 ${talk.id} 的独立链接`}>#{String(talk.id).padStart(3,"0")}</a></footer>
   {#if opened===talk.id}<div id={`talk-comments-${talk.id}`}><TalkComments talkId={talk.id}/></div>{/if}
  </article>
 {/each}</div>
 {#if ready&&items.length<total}<div class="talks-load-more"><button disabled={busy} onclick={()=>load()}>{busy?"正在加载…":"再翻几条日常"}<span aria-hidden="true">↓</span></button></div>{:else if ready&&items.length}<p class="talks-end">— 每个平凡的日子，都值得被记住 —</p>{/if}
</section>
<dialog bind:this={viewer} aria-label="说说图片预览" class="talk-image-viewer"><button onclick={()=>viewer.close()} aria-label="关闭图片预览">关闭 ✕</button>{#if activeImage}<img src={activeImage} alt={activeAlt}/>{/if}</dialog>
<style>
 .talks-feed{display:grid;gap:1.25rem;color:var(--content-text);}.talks-hero{position:relative;overflow:hidden;padding:1.75rem 2rem;border-radius:var(--radius-large);background-image:linear-gradient(125deg,color-mix(in srgb,var(--primary) 8%,transparent),transparent 65%);}.talks-eyebrow{color:var(--primary);font-size:.62rem;letter-spacing:.22em;}.talks-title{display:flex;align-items:center;justify-content:space-between;gap:1rem;margin:.7rem 0;color:var(--primary);}.talks-title h1{font-size:1.85rem;font-weight:600;letter-spacing:.1em;}.talks-hero>p{font-size:.85rem;line-height:1.9;color:var(--content-meta);}.talks-hero-bottom{display:flex;justify-content:space-between;gap:1rem;margin-top:1.4rem;padding-top:1rem;border-top:1px solid var(--line-divider);font-size:.7rem;color:var(--content-meta);}.talks-back{font-size:.8rem;color:var(--primary);}.talks-timeline{display:grid;gap:1.25rem;}.talk-card{padding:1.6rem 1.75rem;border-radius:var(--radius-large);border:1px solid var(--line-divider);box-shadow:0 3px 16px rgb(0 0 0 / 2%);}.talk-card-header{display:flex;align-items:center;gap:.8rem;}.talk-avatar{height:2.6rem;width:2.6rem;border-radius:50%;object-fit:cover;border:2px solid color-mix(in srgb,var(--primary) 18%,transparent);padding:2px;}.talk-author{display:grid;gap:.3rem;min-width:0;}.talk-author strong{font-size:.9rem;font-weight:600;}.talk-author time{font-size:.65rem;color:var(--content-meta);}.talk-pinned{display:flex;align-items:center;gap:.3rem;margin-left:auto;border:1px solid color-mix(in srgb,var(--primary) 20%,transparent);border-radius:99px;padding:.25rem .65rem;color:var(--primary);background:var(--btn-regular-bg);font-size:.65rem;}.talk-content{margin-top:1.15rem;white-space:pre-wrap;overflow-wrap:anywhere;line-height:1.95;font-size:.95rem;}.talk-images{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:.5rem;margin-top:1.1rem;}.talk-images.two-columns{grid-template-columns:repeat(2,minmax(0,1fr));}.talk-images.single{display:block;}.talk-image{display:block;width:100%;border-radius:.9rem;overflow:hidden;background:var(--btn-regular-bg);cursor:zoom-in;}.talk-image img{display:block;width:100%;aspect-ratio:1;object-fit:cover;transition:filter 180ms ease;}.talk-images.single .talk-image{max-width:28rem;}.talk-images.single img{aspect-ratio:auto;max-height:32rem;object-fit:contain;}.talk-card-footer{display:flex;align-items:center;gap:.6rem;margin-top:1.15rem;padding-top:.85rem;border-top:1px solid var(--line-divider);}.talk-action{display:inline-flex;align-items:center;gap:.4rem;padding:.5rem .65rem;border-radius:.8rem;color:var(--content-meta);font-size:.75rem;transition:background 180ms ease,color 180ms ease;}.talk-action:hover,.talk-action[aria-expanded="true"]{background:var(--btn-regular-bg);color:var(--primary);}.talk-action.liked{color:var(--primary);background:var(--btn-regular-bg);}.talk-action:disabled{opacity:.5;}.talk-permalink{margin-left:auto;font-size:.7rem;color:var(--content-meta);}.talks-message,.talks-empty{padding:2rem;border-radius:var(--radius-large);text-align:center;font-size:.85rem;color:var(--content-meta);}.talks-message button{margin-top:1rem;color:var(--primary);}.talks-empty{padding:3.5rem 1rem;}.talks-empty>span{font-size:2.2rem;color:var(--primary);}.talks-empty h2{margin:1rem 0;font-size:1.15rem;}.talks-empty p{font-size:.8rem;}.talks-action-error{color:#c34747;font-size:.8rem;}.talks-load-more{text-align:center;}.talks-load-more button{display:inline-flex;gap:1rem;align-items:center;padding:.85rem 1.4rem;border:1px solid var(--line-divider);border-radius:99px;background:var(--card-bg);font-size:.8rem;color:var(--primary);}.talks-load-more button:disabled{opacity:.5;}.talks-end{padding:1rem 0;text-align:center;font-size:.7rem;color:var(--content-meta);}.talk-image-viewer{max-width:96vw;max-height:95dvh;border-radius:1rem;background:#111;padding:1rem;color:white;}.talk-image-viewer::backdrop{background:rgb(0 0 0 / 75%);}.talk-image-viewer>button{display:block;margin-left:auto;padding:.4rem .7rem;font-size:.85rem;}.talk-image-viewer img{display:block;max-height:80dvh;max-width:100%;margin:auto;object-fit:contain;}button:focus-visible,a:focus-visible{outline:2px solid var(--primary);outline-offset:3px;}@media(max-width:480px){.talks-hero{padding:1.4rem;}.talk-card{padding:1.2rem;}.talks-title h1{font-size:1.6rem;}}@media(hover:hover){.talk-image:hover img{filter:brightness(.92);}}@media(prefers-reduced-motion:reduce){.talk-image img,.talk-action{transition:none;}}
</style>
