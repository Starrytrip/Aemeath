<script lang="ts">
import { onMount } from "svelte";
import { apiFetch } from "@/utils/api-client";
let rows: {
	title: string;
	url: string;
	date: string;
	friend_title: string;
	avatar: string;
}[] = [];
let loading = true;
let error = "";
let failed = 0;
onMount(() => {
	let active = true;
	let timer: ReturnType<typeof setTimeout>;
	async function load() {
		try {
			const r = await apiFetch<{
				items: typeof rows;
				loading: boolean;
				failed_sources: number;
			}>("/friends/feed");
			if (!active) return;
			rows = r.items;
			loading = r.loading;
			failed = r.failed_sources;
			if (loading) timer = setTimeout(load, 2500);
		} catch (e) {
			if (active) {
				error = e instanceof Error ? e.message : "加载失败";
				loading = false;
			}
		}
	}
	void load();
	return () => {
		active = false;
		clearTimeout(timer);
	};
});
</script>
<header class="card-base mb-4 p-6"><h1 class="text-2xl font-bold">朋友圈</h1><p class="mt-3 text-(--content-meta)">友链博客最近的文章</p></header>
{#if error}<p class="card-base p-6" role="alert">{error}</p>{/if}{#if loading}<p class="card-base mb-4 p-6" role="status">正在更新订阅…</p>{/if}{#if failed}<p class="mb-4 text-sm text-(--content-meta)">{failed} 个订阅暂时未能更新。</p>{/if}
<div class="space-y-4">{#each rows as r}<a href={r.url} target="_blank" rel="noopener noreferrer" class="card-base flex items-start gap-4 p-6">{#if r.avatar}<img class="h-12 w-12 rounded-full object-cover" src={r.avatar} alt="" loading="lazy"/>{/if}<div><p class="mb-2 text-sm text-(--content-meta)">{r.friend_title} · {r.date.slice(0,10)}</p><h2 class="text-lg font-bold">{r.title}</h2></div></a>{/each}{#if !loading&&!rows.length&&!error}<p class="card-base p-8">暂无订阅文章。可在后台友链管理填写 RSS 地址。</p>{/if}</div>
