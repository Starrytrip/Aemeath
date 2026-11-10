<script lang="ts">
import { onMount } from "svelte";
import { siteDefaults } from "@/utils/site-defaults";
import { loadSiteSettings } from "@/utils/managed-site";
import {
	loadPublishedArticles,
	articleLink,
	type PublishedArticle,
} from "@/utils/published-articles";
let posts: PublishedArticle[] = [];
let settings = siteDefaults;
let loading = true;
let error = "";
let page = 1;
let layout = siteDefaults.appearance.list_mode;
$: size = Math.trunc(settings.appearance.posts_per_page);
$: pages = Math.max(1, Math.ceil(posts.length / size));
$: visible = posts.slice((page - 1) * size, page * size);
function go(n: number) {
	page = n;
	const u = new URL(location.href);
	u.searchParams.set("page", String(n));
	history.pushState({}, "", u);
}
async function load() {
	loading = true;
	error = "";
	try {
		const [data, config] = await Promise.all([
			loadPublishedArticles(),
			loadSiteSettings().catch(() => null),
		]);
		posts = data;
		settings = config || siteDefaults;
		page = Math.min(
			Math.max(1, Math.ceil(data.length / settings.appearance.posts_per_page)),
			Math.max(
				1,
				Number.parseInt(
					new URLSearchParams(location.search).get("page") ||
						location.pathname.match(/^\/(\d+)\//)?.[1] ||
						"1",
					10,
				) || 1,
			),
		);
		try {
			layout =
				localStorage.getItem("postListLayout") === "list"
					? "list"
					: localStorage.getItem("postListLayout") === "grid"
						? "grid"
						: settings.appearance.list_mode;
		} catch {
			layout = settings.appearance.list_mode;
		}
	} catch (e) {
		error = e instanceof Error ? e.message : "文章加载失败";
	} finally {
		loading = false;
	}
}
onMount(() => {
	void load();
	const pop = () => {
		page = Math.min(
			pages,
			Math.max(
				1,
				Number(new URLSearchParams(location.search).get("page")) || 1,
			),
		);
	};
	const change = () => {
		try {
			layout =
				localStorage.getItem("postListLayout") === "list" ? "list" : "grid";
		} catch {}
	};
	window.addEventListener("popstate", pop);
	window.addEventListener("layoutChange", change);
	return () => {
		window.removeEventListener("popstate", pop);
		window.removeEventListener("layoutChange", change);
	};
});
</script>
{#if loading}<div class="card-base p-8" role="status">正在加载文章…</div>
{:else if error}<div class="card-base p-8" role="alert">{error}<button class="ml-3 text-(--primary)" onclick={load}>重新加载</button></div>
{:else if !posts.length}<div class="card-base p-8">还没有已发布的文章。</div>
{:else}
<div id="post-list-container" class:list-mode={layout==="list"} class:grid-mode={layout==="grid"} class="managed-posts mb-4" style={`--post-card-min-width:340px;`}>
 {#each visible as p (p.slug)}
 <article class={`post-card-wrapper card-base post-card-item ${p.cover_url?"has-cover post-card-full-preview":"no-cover"} ${p.is_pinned?"pinned":""}`}>
  {#if p.cover_url}<a class="managed-cover" href={articleLink(p)} aria-label={p.title}><img src={p.cover_url} alt={p.title} loading="lazy" /></a>{/if}
  <div class="managed-content">
   <h2><a class="post-card-title" href={articleLink(p)}>{p.title}</a></h2>
   <div class="managed-meta">{#if p.is_pinned}<span class="text-(--primary)">⚑ 置顶</span>{/if}<a href={`/archive/?category=${encodeURIComponent(p.category||"")}`}>{p.category||"未分类"}</a>{#each p.tags.slice(0,2) as tag}<a href={`/archive/?tag=${encodeURIComponent(tag)}`}># {tag}</a>{/each}</div>
   <p class="description">{p.description}</p>
   <div class="managed-stats"><time datetime={p.date}>{p.date}</time><span>{p.words.toLocaleString()} 字</span><span>{Math.max(1,Math.ceil(p.words/400))} 分钟</span></div>
  </div>
 </article>
 {/each}
</div>
{#if pages>1}<nav class="flex items-center justify-center gap-4 py-5" aria-label="文章分页"><button disabled={page<=1} onclick={()=>go(page-1)} class="btn-plain rounded-lg p-3 disabled:opacity-40">上一页</button><span>{page} / {pages}</span><button disabled={page>=pages} onclick={()=>go(page+1)} class="btn-plain rounded-lg p-3 disabled:opacity-40">下一页</button></nav>{/if}
{/if}
<style>
.managed-posts{display:grid;gap:1rem;grid-template-columns:repeat(auto-fit,minmax(min(100%,340px),1fr))}.managed-posts.list-mode{grid-template-columns:1fr}.post-card-wrapper{overflow:hidden;display:flex;flex-direction:column;border-radius:var(--radius-large)}.managed-cover{display:block;aspect-ratio:2/1;overflow:hidden}.managed-cover img{width:100%;height:100%;object-fit:cover;transition:transform .25s}.managed-cover:hover img{transform:scale(1.03)}.managed-content{padding:1.15rem;display:flex;flex:1;flex-direction:column;gap:.8rem}.managed-content h2{font-size:1.2rem;line-height:1.5;font-weight:700}.post-card-title:hover{color:var(--primary)}.managed-meta{display:flex;gap:.7rem;flex-wrap:wrap;font-size:.85rem;color:var(--content-meta)}.description{line-height:1.9;color:var(--content-meta);display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden}.managed-stats{margin-top:auto;display:flex;gap:1rem;flex-wrap:wrap;font-size:.8rem;opacity:.6;padding-top:.5rem}:global([data-site-theme="pure"]) .post-card-wrapper{border-bottom:1px solid var(--line-divider)}@media(min-width:780px){.list-mode .post-card-wrapper{flex-direction:row}.list-mode .managed-cover{width:35%;flex-shrink:0;aspect-ratio:4/3}}
</style>
