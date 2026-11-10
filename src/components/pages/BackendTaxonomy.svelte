<script lang="ts">
import { onMount } from "svelte";
import {
	loadPublishedArticles,
	articleTaxonomy,
} from "@/utils/published-articles";
export let kind: "category" | "tag" = "category";
export let compact = false;
let rows: ReturnType<typeof articleTaxonomy> = [];
let loading = true;
let error = "";
onMount(() => {
	let active = true;
	loadPublishedArticles()
		.then((p) => {
			if (active) rows = articleTaxonomy(p, kind);
		})
		.catch((e) => {
			if (active) error = e.message;
		})
		.finally(() => {
			if (active) loading = false;
		});
	return () => {
		active = false;
	};
});
</script>
{#if !compact}<header class="card-base mb-4 px-8 py-6"><h1 class="text-2xl font-bold text-(--primary)">{kind==="tag"?"标签":"分类"}</h1><p class="mt-2 opacity-60">共 {rows.length} 项</p></header>{/if}
{#if loading}<p class="p-4" role="status">正在加载…</p>{:else if error}<p class="p-4" role="alert">{error}</p>{:else}
<div class={compact?"flex flex-wrap gap-2":"card-base flex flex-wrap gap-3 p-6"}>{#each rows as r}<a class="btn-plain flex items-center gap-2 rounded-lg px-3 py-2" href={r.url}><span>{kind==="tag"?"# ":""}{r.name}</span><span class="text-sm opacity-60">{r.count}</span></a>{/each}{#if !rows.length}<p class="p-4 opacity-60">暂无内容</p>{/if}</div>
{/if}
