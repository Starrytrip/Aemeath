<script lang="ts">
import I18nKey from "@i18n/i18nKey";
import { i18n } from "@i18n/translation";
import { onMount } from "svelte";
import Icon from "@/components/common/Icon.svelte";
import type { SearchResult } from "@/global";
import { type ArticleSuggestion, searchPublishedArticles } from "@/utils/navbar-search";
import { url as formatUrl } from "@/utils/url-utils";

export let title = i18n(I18nKey.search);
export let description = "";
type Result = SearchResult & { image?: string; published?: number; category?: string; tags?: string[]; page?: boolean };
type DocumentSearchResponse = {
	mode: "keyword" | "semantic" | "hybrid";
	warning: string | null;
	results: Array<{ document_id: number; title: string; url: string; heading: string; excerpt: string; category: string; tags: string[] }>;
};
let searchSource: "articles" | "documents" = "articles";
let searchNotice = "";
let searchError = "";
let generation = 0;
let activeRequest: AbortController | null = null;
let documentStatus: { documents: number; semantic_configured: boolean; semantic_ready: number } | null = null;
let keyword = "";
let results: Result[] = [];
let total = 0;
let loading = false;
let loadingMore = false;
let ready = false;
let timer: ReturnType<typeof setTimeout>;

const esc = (v: string) => v.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c] || c);
const hi = (v: string, q: string) => {
	const w = q.trim();
	if (!w) return esc(v);
	const e = w.replace(/[.*+?^$()|[\]\\{}]/g, "\\$&");
	return esc(v).replace(new RegExp(e, "gi"), (m) => `<mark>${m}</mark>`);
};
const clean = (v: string) => v.replace(/<[^>]*>/g, "");
const date = (v?: number) => v ? new Intl.DateTimeFormat("zh-CN", {year:"numeric", month:"short", day:"numeric"}).format(new Date(v)) : "";
const articleResult = (item: ArticleSuggestion, q: string): Result => ({
	url: item.url, meta: {title: hi(item.title, q)}, excerpt: hi(item.description || item.excerpt || item.title, q),
	image: item.cover_url, published: Date.parse(`${item.date}T12:00:00`), category: item.category, tags: item.tags,
});

const search = async () => {
	const request = ++generation;
	activeRequest?.abort();
	loadingMore = false;
	searchNotice = "";
	searchError = "";
	const q = keyword.trim();
	if (!ready || !q) { results = []; total = 0; loading = false; return; }
	const controller = new AbortController();
	activeRequest = controller;
	loading = true;
	try {
		if (searchSource === "documents") {
			const response = await fetch(`/api/v1/search?q=${encodeURIComponent(q)}&mode=hybrid&limit=20`, { cache: "no-store", signal: AbortSignal.any([controller.signal, AbortSignal.timeout(40_000)]) });
			if (!response.ok) throw new Error("文档搜索暂不可用，请稍后重试。");
			const data: DocumentSearchResponse = await response.json();
			if (request !== generation) return;
			results = data.results.map((item) => ({ url: item.url, meta: {title: hi(item.title,q)}, excerpt: hi([item.heading,item.excerpt].filter(Boolean).join(" · "),q), category:item.category,tags:item.tags }));
			total = results.length;
			searchNotice = data.warning || (data.mode === "hybrid" ? "按关键词与语义综合排序" : data.mode === "semantic" ? "按语义相关度排序" : "按关键词相关度排序");
		} else {
			const data = await searchPublishedArticles(q, { limit: 20, signal: controller.signal });
			if (request !== generation) return;
			results = data.items.map((item) => articleResult(item,q));
			total = data.total;
		}
	} catch {
		if (request !== generation) return;
		results = []; total = 0;
		searchError = searchSource === "documents" ? "文档搜索暂不可用，请稍后重试。" : "文章搜索暂不可用，请稍后重试。";
	} finally { if (request === generation) loading = false; }
};

const loadMore = async () => {
	if (loading || loadingMore || searchSource !== "articles" || results.length >= total) return;
	const request = generation;
	const q = keyword.trim();
	const controller = new AbortController();
	activeRequest = controller;
	loadingMore = true;
	searchError = "";
	try {
		const data = await searchPublishedArticles(q, { limit:20, offset:results.length, signal:controller.signal });
		if (request !== generation) return;
		results = [...results, ...data.items.map((item) => articleResult(item,q)).filter((item) => !results.some((old) => old.url === item.url))];
		total = data.total;
	} catch {
		if (request === generation) searchError = "更多文章加载失败，请重试。";
	} finally { if (request === generation) loadingMore = false; }
};
const changeSource = (source: "articles" | "documents") => { searchSource = source; clearTimeout(timer); void search(); };
onMount(() => {
	ready = true;
	keyword = new URLSearchParams(location.search).get("q") || "";
	if (keyword) void search();
	void fetch("/api/v1/search/status", { signal: AbortSignal.timeout(5000) }).then(async (response) => { if (response.ok) documentStatus = await response.json(); }).catch(() => {});
	return () => {clearTimeout(timer); generation++; activeRequest?.abort();};
});
const input = () => { clearTimeout(timer); generation++; activeRequest?.abort(); loadingMore = false; timer = setTimeout(search, searchSource === "documents" ? 800 : 260); };
</script>

<section class="search-page" aria-labelledby="search-page-title">
 <header class="hero">
  <p class="eyebrow"><i></i>RAIN'S ARCHIVE</p>
  <h1 id="search-page-title">{title}</h1>
  <p>{description || "从项目复盘、博客改造到零散思考，把写过的内容重新找回来。"}</p>
  <div class="mt-5 flex flex-wrap items-center gap-2" role="group" aria-label="搜索范围">
   <button type="button" class="cursor-pointer rounded-full border border-(--line-divider) px-4 py-2 text-sm text-(--content-meta) aria-pressed:border-(--primary) aria-pressed:bg-(--btn-regular-bg) aria-pressed:text-(--primary)" aria-pressed={searchSource === "articles"} on:click={() => changeSource("articles")}>文章搜索</button>
   <button type="button" class="cursor-pointer rounded-full border border-(--line-divider) px-4 py-2 text-sm text-(--content-meta) aria-pressed:border-(--primary) aria-pressed:bg-(--btn-regular-bg) aria-pressed:text-(--primary)" aria-pressed={searchSource === "documents"} on:click={() => changeSource("documents")}>文档搜索</button>
   {#if searchSource === "documents" && documentStatus}<span class="text-xs text-(--content-meta)">已收录 {documentStatus.documents} 篇{documentStatus.semantic_configured ? ` · ${documentStatus.semantic_ready} 篇支持语义搜索` : " · 关键词搜索"}</span>{/if}
  </div>
  <div class="field">
   <label class="sr-only" for="search-input">{title}</label><Icon icon="material-symbols:search-rounded" />
   <input id="search-input" type="search" placeholder={searchSource === "documents" ? "描述你想找的内容，例如：如何降低后端内存" : "输入关键词，搜索已发布文章"} maxlength="200" bind:value={keyword} on:input={input} />
   <small>{keyword ? (loading ? "正在检索" : "找到 "+total+" 条") : "试试：AI、自动上架、博客"}</small>
  </div>
 </header>
 {#if searchNotice}<p class="mt-4 text-sm text-(--content-meta)" role="status">{searchNotice}</p>{/if}
 {#if searchError}<p class="mt-4 text-sm text-red-500" role="alert">{searchError}</p>{/if}
 <div class="result-head">{#if keyword && !loading && results.length}<p><strong>{total}</strong> 条和「{keyword}」有关的记录</p><span>{searchSource === "articles" ? "已发布文章按相关度排列" : "文档按相关度排列"}</span>{:else if !keyword}<p>输入一个词，开始翻找这座小小的内容档案室。</p>{/if}</div>
 {#if loading}
  <div class="empty"><Icon icon="svg-spinners:ring-resize" /><p>正在从文章里寻找线索…</p></div>
 {:else if results.length}
  <div class="list">{#each results as result, index}
   <article class="card" style={"--i:"+index}>
    <a href={result.url}>
     <div class:fallback={!result.image} class="cover">{#if result.image}<img src={formatUrl(result.image)} alt="" loading={index>1?"lazy":"eager"} />{:else}<b>{clean(result.meta.title).slice(0,1)||"R"}</b><Icon icon="material-symbols:article-rounded" />{/if}</div>
     <div class="body">
      <div class="meta"><span>{result.category || (result.page ? "页面" : "文章")}</span><time>{result.published ? date(result.published) : (result.page ? "站内页面" : "博客记录")}</time></div>
      <h2>{@html result.meta.title}</h2><p class="excerpt">{@html result.excerpt || "这条记录暂时没有摘要，点进去看看完整内容。"}</p>
      <footer><div>{#each (result.tags||[]).slice(0,3) as tag}<em>{tag}</em>{/each}</div><b>↗</b></footer>
     </div>
    </a>
   </article>
  {/each}</div>
  {#if searchSource === "articles" && results.length < total}<div class="mt-6 text-center"><button type="button" class="rounded-full border border-(--line-divider) bg-(--card-bg) px-6 py-3 text-sm text-(--primary)" disabled={loadingMore} on:click={loadMore}>{loadingMore ? "正在加载…" : "加载更多文章"}</button></div>{/if}
 {:else if keyword}
  <div class="empty"><Icon icon="material-symbols:travel-explore-rounded" /><h2>暂时没找到这条线索</h2><p>换一个更短的关键词试试，或从“AI”“博客”“自动化”这类主题开始。</p></div>
 {:else}
  <div class="empty"><Icon icon="material-symbols:auto-stories-rounded" /><h2>从一段关键词开始</h2><p>{searchSource === "documents" ? "搜索后端已收录的文档正文与章节；启用语义搜索后也能按意思查找。" : "搜索已发布文章的标题、摘要、分类、标签与正文。"}</p></div>
 {/if}
</section>

<style>
.search-page{--ink:color-mix(in oklab,var(--text-color,#26344d) 88%,#17233a);--muted:color-mix(in oklab,var(--text-color,#65738b) 57%,transparent);--line:color-mix(in oklab,var(--primary) 15%,var(--line-divider,#dfe7f3));padding-bottom:1rem}
.hero{position:relative;overflow:hidden;padding:clamp(1.45rem,3.6vw,2.65rem);border:1px solid var(--line);border-radius:clamp(1.25rem,2.6vw,1.9rem);background:linear-gradient(125deg,color-mix(in oklab,var(--card-bg) 89%,#eaf7ff 11%),var(--card-bg));box-shadow:0 1.1rem 2.8rem color-mix(in oklab,var(--primary) 10%,transparent)}
.hero:after{content:"";position:absolute;top:-11rem;right:-5rem;width:22rem;height:22rem;border-radius:50%;background:radial-gradient(circle,color-mix(in oklab,var(--primary) 23%,transparent),transparent 67%);pointer-events:none}.eyebrow{display:flex;gap:.55rem;align-items:center;margin:0 0 .7rem;color:var(--primary);font:800 .68rem/1 ui-monospace,monospace;letter-spacing:.18em}.eyebrow i{width:.48rem;height:.48rem;border-radius:50%;background:linear-gradient(135deg,var(--primary),#6ed5ce);box-shadow:0 0 0 .25rem color-mix(in oklab,var(--primary) 10%,transparent)}.hero h1{margin:0;color:var(--ink);font:600 clamp(2.25rem,4.6vw,4rem)/1 Georgia,"Noto Serif SC",serif;letter-spacing:-.055em}.hero>p:not(.eyebrow){max-width:34rem;margin:.85rem 0 0;color:var(--muted);font-size:.92rem;line-height:1.8}
.field{display:flex;position:relative;align-items:center;gap:.7rem;margin-top:clamp(1.45rem,3vw,2rem);padding:.42rem .5rem .42rem 1rem;border:1px solid color-mix(in oklab,var(--primary) 17%,var(--line-divider));border-radius:1rem;background:color-mix(in oklab,var(--card-bg) 86%,transparent);box-shadow:inset 0 1px 0 rgb(255 255 255/.32),0 .7rem 1.7rem rgb(65 117 171/.08);transition:.22s}.field:focus-within{border-color:color-mix(in oklab,var(--primary) 58%,transparent);box-shadow:0 0 0 .24rem color-mix(in oklab,var(--primary) 11%,transparent);transform:translateY(-1px)}.field :global(svg){flex:0 0 auto;color:var(--primary);font-size:1.35rem}.field input{min-width:0;flex:1;border:0;outline:0;background:transparent;color:var(--ink);font-size:.94rem;line-height:2.35rem}.field small{padding:.42rem .65rem;border-radius:.66rem;color:color-mix(in oklab,var(--primary) 80%,var(--ink));background:color-mix(in oklab,var(--primary) 9%,transparent);font:700 .7rem/1.15 ui-monospace,monospace;white-space:nowrap}
.result-head{display:flex;min-height:3.9rem;align-items:end;justify-content:space-between;gap:1rem;padding:1.8rem 0 .8rem;color:var(--muted);font-size:.82rem}.result-head p{margin:0}.result-head strong{margin-right:.25rem;color:var(--primary);font-size:1.08rem}.result-head span{font-size:.72rem}.list{display:grid;gap:.85rem}.card{overflow:hidden;border:1px solid var(--line);border-radius:1.25rem;background:color-mix(in oklab,var(--card-bg) 90%,transparent);box-shadow:0 .7rem 1.8rem rgb(57 90 139/.055);animation:in .46s both;animation-delay:calc(var(--i)*45ms);transition:.25s}.card:hover{border-color:color-mix(in oklab,var(--primary) 34%,transparent);box-shadow:0 1rem 2.25rem rgb(57 90 139/.13);transform:translateY(-3px)}.card>a{display:grid;grid-template-columns:clamp(8.4rem,20vw,12.5rem) minmax(0,1fr);min-height:8.6rem;color:inherit;text-decoration:none}.cover{position:relative;display:grid;overflow:hidden;place-items:center;background:linear-gradient(145deg,#8bc9df,#789bd9 58%,#e9afce);isolation:isolate}.cover:after{content:"";position:absolute;inset:0;background:linear-gradient(110deg,rgb(13 32 68/.08),transparent 55%,rgb(13 32 68/.2))}.cover img{width:100%;height:100%;object-fit:cover;transition:transform .65s cubic-bezier(.2,.7,.2,1)}.card:hover img{transform:scale(1.075)}.fallback b{z-index:1;color:#fff;font:600 clamp(2.5rem,5vw,4.6rem)/1 Georgia,serif}.fallback :global(svg){position:absolute;right:.85rem;bottom:.75rem;z-index:1;color:rgb(255 255 255/.6);font-size:1.35rem}.body{display:flex;min-width:0;flex-direction:column;padding:1.05rem 1.25rem 1rem}.meta{display:flex;gap:.55rem;color:var(--muted);font:700 .68rem/1.1 ui-monospace,monospace}.meta span{color:color-mix(in oklab,var(--primary) 82%,var(--ink))}.meta span:before{content:"";display:inline-block;width:.35rem;height:.35rem;margin-right:.36rem;border-radius:50%;background:currentColor}.meta time:before{content:"·";margin-right:.55rem}.body h2{display:-webkit-box;overflow:hidden;margin:.48rem 0 0;color:var(--ink);font:600 clamp(1.13rem,2.2vw,1.42rem)/1.26 Georgia,"Noto Serif SC",serif;letter-spacing:-.035em;-webkit-box-orient:vertical;-webkit-line-clamp:2;transition:color .22s}.card:hover h2{color:var(--primary)}.excerpt{display:-webkit-box;overflow:hidden;margin:.48rem 0 0;color:var(--muted);font-size:.81rem;line-height:1.63;-webkit-box-orient:vertical;-webkit-line-clamp:2}.body footer{display:flex;align-items:center;justify-content:space-between;gap:.7rem;margin-top:auto;padding-top:.75rem}.body footer div{display:flex;min-width:0;gap:.35rem;overflow:hidden}.body footer em{overflow:hidden;max-width:8.8rem;padding:.28rem .48rem;border-radius:.45rem;background:color-mix(in oklab,var(--primary) 6%,transparent);color:var(--muted);font-size:.66rem;font-style:normal;line-height:1;text-overflow:ellipsis;white-space:nowrap}.body footer>b{display:grid;width:1.85rem;height:1.85rem;place-items:center;border:1px solid color-mix(in oklab,var(--primary) 16%,transparent);border-radius:50%;color:var(--primary);background:color-mix(in oklab,var(--primary) 6%,transparent);transition:.25s}.card:hover footer>b{color:#fff;background:var(--primary);transform:translate(.16rem,-.16rem)}.empty{display:flex;min-height:17rem;flex-direction:column;align-items:center;justify-content:center;padding:2rem;border:1px dashed color-mix(in oklab,var(--primary) 22%,var(--line-divider));border-radius:1.25rem;background:color-mix(in oklab,var(--card-bg) 62%,transparent);color:var(--muted);text-align:center}.empty :global(svg){color:var(--primary);font-size:2rem}.empty h2{margin:1rem 0 0;color:var(--ink);font:600 1.38rem/1.2 Georgia,"Noto Serif SC",serif}.empty p{max-width:23rem;margin:.55rem 0 0;font-size:.83rem;line-height:1.75}:global(.card mark){padding:0 .08em;border-radius:.18em;color:var(--primary);background:color-mix(in oklab,var(--primary) 11%,transparent);font-weight:700}@keyframes in{from{opacity:0;transform:translateY(.75rem)}to{opacity:1;transform:translateY(0)}}@media(max-width:640px){.hero{padding:1.35rem}.field{gap:.45rem;padding-left:.78rem}.field small{display:none}.result-head{min-height:3.4rem;padding-top:1.25rem}.result-head span{display:none}.card>a{grid-template-columns:6.6rem minmax(0,1fr);min-height:7.9rem}.body{padding:.82rem .9rem}.body footer div{display:none}.body footer{padding-top:.48rem}}@media(prefers-reduced-motion:reduce){.card,.cover img,.body footer>b,.field{animation:none;transition:none}}
/* Search stays on the homepage typography system, not the portfolio's serif display type. */
.hero h1,
.fallback b,
.body h2,
.empty h2 {
	font-family: var(--font-active-sans) !important;
}

.hero h1 { font-weight: 700; line-height: 1.1; letter-spacing: -.04em; }
.fallback b { font-weight: 700; }
.body h2 { font-weight: 700; line-height: 1.35; letter-spacing: -.025em; }
.empty h2 { font-weight: 700; line-height: 1.35; }
</style>
