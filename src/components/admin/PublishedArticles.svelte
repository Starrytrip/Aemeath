<script lang="ts">
import { onMount, tick } from "svelte";
import GithubSlugger from "github-slugger";
import Icon from "@/components/common/Icon.svelte";
import { typewriterLifecycle } from "@/scripts/typewriter";
import { apiFetch } from "@/utils/api-client";
import { url } from "@/utils/url-utils";
export let reader = false;
export let sourceKey = "";
interface Item {
	title: string;
	slug: string;
	description: string;
	ai_summary?: string;
	date: string;
	cover_url: string;
	category: string;
	is_pinned: boolean;
}
interface Article {
	title: string;
	html: string;
	source_key: string;
	category: string;
	tags: string[];
	meta: Item;
}
let items: Item[] = [];
let article: Article | null = null;
let error = "";
let loading = true;
let controller: AbortController | null = null;
let disposed = false;
let headings: { id: string; text: string; depth: number }[] = [];

function prepareArticleHtml(data: Article): string {
	const template = document.createElement("template");
	template.innerHTML = data.html;
	for (const img of template.content.querySelectorAll("img"))
		img.loading = "lazy";
	const slugger = new GithubSlugger();
	headings = [
		...template.content.querySelectorAll<HTMLElement>("h1,h2,h3,h4,h5,h6"),
	].map((heading, index) => {
		heading.id = slugger.slug(heading.textContent || `section-${index + 1}`);
		return {
			id: heading.id,
			text: heading.textContent || "",
			depth: Number(heading.tagName.slice(1)),
		};
	});
	return template.innerHTML;
}
async function load() {
	controller?.abort();
	const request = new AbortController();
	controller = request;
	loading = true;
	error = "";
	article = null;
	try {
		const signal = AbortSignal.any([
			request.signal,
			AbortSignal.timeout(15000),
		]);
		if (reader) {
			const slug = new URLSearchParams(location.search).get("slug");
			if (!sourceKey && !slug) throw new Error("缺少文章标识");
			const endpoint = sourceKey
				? `/article-source?${new URLSearchParams({ source_key: sourceKey })}`
				: `/articles/${encodeURIComponent(slug || "")}`;
			const data = await apiFetch<Article>(endpoint, {
				cache: "no-store",
				signal,
			});
			data.html = prepareArticleHtml(data);
			if (disposed || controller !== request) return;
			article = data;
			document.title = `${data.title} - Yukina`;
		} else {
			const data = await apiFetch<Item[]>("/articles", {
				cache: "no-store",
				signal,
			});
			if (disposed || controller !== request) return;
			items = data;
		}
	} catch (e) {
		if (disposed || controller !== request) return;
		error =
			e instanceof Error && "status" in e && e.status === 404
				? "文章不存在或已下架。"
				: e instanceof Error
					? e.message
					: "文章加载失败，请稍后重试。";
	} finally {
		if (!disposed && controller === request) {
			loading = false;
			await tick();
			if (article)
				document.dispatchEvent(new CustomEvent("yukina:article-loaded"));
			if (article && location.hash) {
				try {
					document
						.getElementById(decodeURIComponent(location.hash.slice(1)))
						?.scrollIntoView();
				} catch {
					/* Invalid fragment. */
				}
			}
		}
	}
}
onMount(() => {
	void load();
	return () => {
		disposed = true;
		controller?.abort();
	};
});
</script>

{#if loading}
	<div class="card-base p-10 text-center text-(--content-meta)" role="status">正在加载文章…</div>
{:else if error}
	<div class="card-base p-10 text-center"><p role="alert" class="text-(--content-meta)">{error}</p><button type="button" class="mt-5 rounded-xl bg-(--btn-regular-bg) px-5 py-2 text-(--primary)" on:click={load}>重新加载</button></div>
{:else if article}
	{#if article.meta.ai_summary?.trim()}
		<div class="post-ai-summary-shell">
			<aside class="banner-post-ai-summary post-ai-summary-inline mobile-post-ai-summary" aria-label="AI摘要">
				<div class="banner-post-ai-summary__label"><Icon icon="material-symbols:auto-awesome-rounded" /><span>AI摘要</span></div>
				<p><span class="sr-only">{article.meta.ai_summary}</span><span class="typewriter banner-post-ai-summary__typewriter" data-text={article.meta.ai_summary} data-speed="20" aria-hidden="true" use:typewriterLifecycle></span></p>
				<div class="banner-post-ai-summary__disclaimer">内容由 AI 辅助整理，请结合正文阅读并注意甄别。</div>
			</aside>
		</div>
	{/if}
	<article class="card-base mb-4 p-6 md:p-10" data-backend-article>
		<a href={url("/articles/")} class="text-sm text-(--primary)">← 文章列表</a>
		<header class="my-6 border-b border-(--line-divider) pb-6">
			<h1 class="text-3xl font-bold leading-snug text-(--content-text) md:text-4xl">{article.title}</h1>
			<div class="mt-4 flex flex-wrap gap-3 text-sm text-(--content-meta)">
				{#if article.meta.date}<time datetime={article.meta.date}>{article.meta.date}</time>{/if}
				{#if article.category}<span>{article.category}</span>{/if}
				{#if article.meta.is_pinned}<span class="text-(--primary)">置顶</span>{/if}
				{#each article.tags || [] as tag}<span>#{tag}</span>{/each}
			</div>
			{#if article.meta.description}<p class="mt-4 leading-relaxed text-(--content-meta)">{article.meta.description}</p>{/if}
		</header>
		{#if article.meta.cover_url}<img class="mb-8 max-h-[32rem] w-full rounded-xl object-cover" src={url(article.meta.cover_url)} alt="文章封面" />{/if}
		{#if headings.length}<details class="mb-6 rounded-xl border border-(--line-divider) p-4" data-theme-only="pure"><summary class="cursor-pointer text-(--primary)">文章目录</summary><nav class="mt-3 flex flex-col gap-2 text-sm" aria-label="文章目录">{#each headings as heading}<a href={`#${heading.id}`} style={`padding-left:${Math.max(0, heading.depth - 1)}rem`}>{heading.text}</a>{/each}</nav></details>{/if}
		<div class="prose custom-md markdown-content max-w-none dark:prose-invert">{@html article.html}</div>
	</article>
	<slot />
{:else}
	<h1 class="mb-6 text-2xl font-bold">已发布文章</h1>
	<div class="grid gap-5 md:grid-cols-2">
		{#each items as item}
			<a class="card-base overflow-hidden" href={url(`/reader/?slug=${encodeURIComponent(item.slug)}`)}>
				{#if item.cover_url}<img class="aspect-video w-full object-cover" src={url(item.cover_url)} alt="" />{/if}
				<div class="p-6"><p class="mb-2 text-xs text-(--content-meta)">{[item.date, item.category, item.is_pinned ? "置顶" : ""].filter(Boolean).join(" · ")}</p><h2 class="text-xl font-semibold">{item.title}</h2><p class="mt-3 text-sm text-(--content-meta)">{item.description}</p></div>
			</a>
		{/each}
	</div>
	{#if !items.length}<p class="card-base p-10 text-(--content-meta)">暂无后台发布的文章。</p>{/if}
{/if}

<style>
	.typewriter { position: relative; }
	.typewriter::after { content: "|"; margin-left: 2px; animation: blink 1s infinite; }
	@keyframes blink { 0%, 50% { opacity: 1; } 51%, 100% { opacity: 0; } }
	@media (prefers-reduced-motion: reduce) { .typewriter::after { display: none; } }
</style>
