<script lang="ts">
import { onMount } from "svelte";
import Icon from "@/components/common/Icon.svelte";
import {
	type ArticleSuggestion,
	searchNavbarArticles,
} from "@/utils/navbar-search";
import { navigateToPage } from "@/utils/navigation-utils";
import { getSearchUrl, url } from "@/utils/url-utils";

let root: HTMLDivElement;
let query = "";
let results: ArticleSuggestion[] = [];
let open = false;
let loading = false;
let error = "";
let active = -1;
let request = 0;
let timer: ReturnType<typeof setTimeout>;
let activeRequest: AbortController | null = null;

function highlightParts(text: string, keyword: string) {
	const terms = [...new Set(keyword.trim().split(/\s+/).filter(Boolean))]
		.sort((a, b) => b.length - a.length)
		.map((term) => term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
	if (!terms.length) return [{ text, matched: false }];
	const parts: { text: string; matched: boolean }[] = [];
	let position = 0;
	for (const match of text.matchAll(new RegExp(terms.join("|"), "gi"))) {
		const start = match.index;
		if (start > position)
			parts.push({ text: text.slice(position, start), matched: false });
		parts.push({ text: match[0], matched: true });
		position = start + match[0].length;
	}
	if (position < text.length)
		parts.push({ text: text.slice(position), matched: false });
	return parts;
}

function close() {
	open = false;
	loading = false;
	active = -1;
	request++;
	clearTimeout(timer);
	activeRequest?.abort();
	activeRequest = null;
}

function search(value: string) {
	query = value;
	clearTimeout(timer);
	activeRequest?.abort();
	const token = ++request;
	results = [];
	active = -1;
	error = "";
	open = !!query.trim();
	loading = open;
	if (!open) return;
	timer = setTimeout(async () => {
		const controller = new AbortController();
		activeRequest = controller;
		try {
			const matches = await searchNavbarArticles(value, controller.signal);
			if (token === request) results = matches;
		} catch {
			if (token === request) error = "暂时无法加载建议，可进入搜索页重试。";
		} finally {
			if (token === request) loading = false;
		}
	}, 180);
}

function blurSearch() {
	if (
		document.activeElement instanceof HTMLElement &&
		root.contains(document.activeElement)
	)
		document.activeElement.blur();
}

function navigate(destination: string) {
	close();
	blurSearch();
	navigateToPage(destination);
}

function submit(event: SubmitEvent) {
	event.preventDefault();
	navigate(query.trim() ? getSearchUrl(query) : url("/search/"));
}

function keydown(event: KeyboardEvent) {
	if (event.isComposing) return;
	if (event.key === "Escape") {
		event.preventDefault();
		close();
		blurSearch();
	} else if (
		(event.key === "ArrowDown" || event.key === "ArrowUp") &&
		results.length
	) {
		event.preventDefault();
		open = true;
		active =
			active < 0
				? event.key === "ArrowDown"
					? 0
					: results.length - 1
				: (active + (event.key === "ArrowDown" ? 1 : -1) + results.length) %
					results.length;
	} else if (event.key === "Enter" && open && active >= 0 && results[active]) {
		event.preventDefault();
		navigate(results[active].url);
	}
}

function follow(event: MouseEvent, destination: string) {
	if (
		event.ctrlKey ||
		event.metaKey ||
		event.shiftKey ||
		event.altKey ||
		event.button !== 0
	)
		return;
	event.preventDefault();
	navigate(destination);
}

onMount(() => {
	const outside = (event: PointerEvent) => {
		if (event.target instanceof Node && !root.contains(event.target)) {
			close();
			blurSearch();
		}
	};
	const leave = () => close();
	document.addEventListener("pointerdown", outside);
	document.addEventListener("astro:before-swap", leave);
	window.addEventListener("popstate", leave);
	return () => {
		close();
		document.removeEventListener("pointerdown", outside);
		document.removeEventListener("astro:before-swap", leave);
		window.removeEventListener("popstate", leave);
	};
});
</script>

{#snippet highlight(text: string)}
	{#each highlightParts(text, query) as part}
		{#if part.matched}<mark>{part.text}</mark>{:else}{part.text}{/if}
	{/each}
{/snippet}

<div class="navbar-search" bind:this={root} onfocusout={(event) => { if (!(event.relatedTarget instanceof Node) || !root.contains(event.relatedTarget)) close(); }}>
	<form class="navbar-search-form" action={url("/search/")} role="search" aria-label="导航文章搜索" onsubmit={submit}>
		<input id="navbar-article-query" type="search" name="q" placeholder="搜索文章…" aria-label="搜索文章" maxlength="200" autocomplete="off" role="combobox" aria-autocomplete="list" aria-expanded={open} aria-controls="navbar-article-suggestions" aria-activedescendant={open && active >= 0 ? `navbar-suggestion-${active}` : undefined} value={query} oninput={(event) => { if (!event.isComposing) search(event.currentTarget.value); }} oncompositionend={(event) => search(event.currentTarget.value)} onfocus={() => { if (query.trim()) search(query); }} onkeydown={keydown} />
		<button type="submit" aria-label="搜索" title="进入搜索页面"><Icon icon="material-symbols:search-rounded" class="text-[1.2rem]" /></button>
	</form>
	{#if open && query.trim()}
		<div class="navbar-search-dropdown custom-scrollbar">
			<div class="navbar-search-caption" role="status">{loading ? "正在查找文章…" : error || (results.length ? "相关文章" : "没有找到相关文章，换个关键词试试。")}</div>
			<ul role="listbox" id="navbar-article-suggestions" aria-label="文章搜索建议">
				{#each results as post, index (post.url)}
					<li role="option" id={`navbar-suggestion-${index}`} aria-selected={active === index}>
						<a href={post.url} onclick={(event) => follow(event, post.url)}>
							<strong>{@render highlight(post.title)}</strong>
							{#if post.description}<span>{@render highlight(post.description)}</span>{/if}
							{#if post.category}<small>{@render highlight(post.category)}</small>{/if}
						</a>
					</li>
				{/each}
			</ul>
			<a class="navbar-search-all" href={getSearchUrl(query)} onclick={(event) => follow(event, getSearchUrl(query))}>进入搜索页查看全部结果 →</a>
		</div>
	{/if}
</div>

<style>
	.navbar-search { position: relative; flex: 0 0 auto; width: clamp(10rem, 12vw, 14rem); color: var(--nav-fg-1, var(--content-text)); }
	.navbar-search-form { display: flex; align-items: center; height: 44px; border: 1px solid var(--navbar-module-border, var(--line-divider)); border-radius: 999px; background: var(--navbar-module-bg, var(--card-bg)); box-shadow: var(--navbar-module-shadow); backdrop-filter: blur(24px); }
	input { width: 100%; min-width: 0; height: 100%; padding: 0 0.25rem 0 0.9rem; background: transparent; color: inherit; font-size: .8rem; outline: none; border: 0; border-radius: 999px 0 0 999px; }
	input::placeholder { color: var(--nav-fg-2, var(--content-meta)); }
	button { display: flex; flex: 0 0 42px; justify-content: center; align-items: center; height: 42px; border-radius: 50%; color: var(--primary); }
	button:hover { background: var(--btn-regular-bg); }
	.navbar-search-form:focus-within { border-color: var(--primary); }
	button:focus-visible, a:focus-visible { outline: 2px solid var(--primary); outline-offset: 1px; }
	.navbar-search-dropdown { position: absolute; top: calc(100% + .7rem); right: 0; z-index: 100; width: min(26rem, calc(100vw - 2rem)); max-height: min(32rem, calc(100dvh - 6rem)); overflow: auto; overscroll-behavior: contain; padding: .65rem; border: 1px solid var(--line-divider); border-radius: 1rem; background: var(--float-panel-bg, var(--card-bg)); box-shadow: 0 12px 36px rgb(0 0 0 / 12%); color: var(--content-text); }
	.navbar-search-caption { padding: .4rem .5rem .65rem; color: var(--content-meta); font-size: .7rem; }
	ul { margin: 0; padding: 0; list-style: none; }
	li > a { display: grid; gap: .3rem; padding: .7rem .6rem; border-radius: .65rem; }
	li[aria-selected="true"] > a, li > a:hover { background: var(--btn-regular-bg); }
	strong { font-size: .85rem; line-height: 1.5; font-weight: 600; overflow-wrap: anywhere; }
	mark { color: var(--primary); background: color-mix(in srgb, var(--primary) 18%, transparent); border-radius: .2em; padding: 0 .05em; font-weight: 800; }
	li span { display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; font-size: .7rem; line-height: 1.7; color: var(--content-meta); }
	small { font-size: .65rem; color: var(--primary); }
	.navbar-search-all { display: block; margin-top: .5rem; padding: .65rem; border-top: 1px solid var(--line-divider); text-align: center; font-size: .75rem; color: var(--primary); }
	@media(max-width: 767px) {
		.navbar-search { width: clamp(4.5rem, 21vw, 6rem); }
		.navbar-search-form { height: 36px; }
		input { padding-left: .5rem; font-size: .7rem; }
		button { height: 34px; flex-basis: 30px; }
		.navbar-search:focus-within { position: static; }
		.navbar-search:focus-within .navbar-search-form { position: absolute; top: .45rem; left: 1rem; right: 1rem; z-index: 110; background: var(--float-panel-bg, var(--card-bg)); }
		.navbar-search-dropdown { position: fixed; top: 4.5rem; left: .75rem; right: .75rem; width: auto; }
	}
</style>
