<script lang="ts">
import { onMount } from "svelte";
import { apiFetch } from "@/utils/api-client";
import type { ManagedFriend } from "@/types/managed-site";
export let directory = false;
let rows: ManagedFriend[] = [];
let loading = true;
let error = "";
onMount(() => {
	let active = true;
	apiFetch<{ items: ManagedFriend[] }>("/friends")
		.then((r) => {
			if (active) rows = r.items;
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
{#if loading}<p class="card-base p-6" role="status">正在加载友链…</p>{:else if error}<p class="card-base p-6" role="alert">{error}</p>{:else}<div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">{#each rows as f}<a class="card-base flex items-center gap-4 rounded-2xl p-5 transition hover:-translate-y-1" href={f.siteurl} target="_blank" rel="noopener noreferrer">{#if f.imgurl}<img src={f.imgurl} alt="" class="h-14 w-14 rounded-full object-cover" loading="lazy"/>{/if}<div class="min-w-0"><h2 class="font-bold text-(--content-text)">{f.title}{#if f.recommended}<span class="ml-2 text-sm text-(--primary)">推荐</span>{/if}</h2><p class="mt-2 text-sm leading-6 text-(--content-meta)">{f.description}</p>{#if directory&&f.rss}<span class="mt-2 block text-xs text-(--primary)">已配置 RSS</span>{/if}</div></a>{/each}{#if !rows.length}<p class="card-base p-8 text-(--content-meta)">还没有友链</p>{/if}</div>{/if}
