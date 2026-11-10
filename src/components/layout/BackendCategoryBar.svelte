<script lang="ts">
import { onMount } from "svelte";
import {
	articleTaxonomy,
	loadPublishedArticles,
} from "@/utils/published-articles";
import Icon from "@/components/common/Icon.svelte";
let rows: ReturnType<typeof articleTaxonomy> = [];
let total: number | null = null;
onMount(() => {
	let active = true;
	loadPublishedArticles()
		.then((p) => {
			if (active) {
				total = p.length;
				rows = articleTaxonomy(p, "category");
			}
		})
		.catch(() => {});
	return () => {
		active = false;
	};
});
</script>
<nav id="category-bar" class="card-base category-bar mb-4 flex items-center gap-2 overflow-x-auto p-3" aria-label="文章分类">
<a class="category-pill btn-plain shrink-0 rounded-xl px-3 py-2" href="/" aria-label="主页"><Icon icon="material-symbols:home"/></a>
<a class="category-pill btn-plain flex shrink-0 items-center gap-2 rounded-xl px-3 py-2" href="/archive/">归档 {#if total!==null}<span class="text-xs opacity-60">{total}</span>{/if}</a>
{#each rows as r}<a class="category-pill btn-plain flex shrink-0 items-center gap-2 whitespace-nowrap rounded-xl px-3 py-2" href={r.url}>{r.name}<span class="text-xs opacity-60">{r.count}</span></a>{/each}<a class="category-pill btn-plain shrink-0 whitespace-nowrap rounded-xl px-3 py-2" href="/categories/">更多</a>
</nav>
<style>nav{scrollbar-width:none}nav::-webkit-scrollbar{display:none}</style>
