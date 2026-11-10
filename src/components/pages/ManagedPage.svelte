<script lang="ts">
import { onMount } from "svelte";
import { apiFetch } from "@/utils/api-client";
export let slug = "about";
export let embedded=false;
let title = "";
let html = "";
let loading = true;
let error = "";
onMount(() => {
	let active = true;
	apiFetch<{ title: string; html: string }>(
		`/pages/${encodeURIComponent(slug)}`,
	)
		.then((r) => {
			if (active) {
				title = r.title;
				html = r.html;
				if(!embedded)document.title = `${r.title} · ${window.__yukinaSiteDefaults?.basic.title || "Yukina"}`;
			}
		})
		.catch((e) => {
			if (active) error = e.status === 404 ? "页面未发布或已下架" : e.message;
		})
		.finally(() => {
			if (active) loading = false;
		});
	return () => {
		active = false;
	};
});
</script>
<article class={embedded?"":"card-base p-6 md:p-9"}>{#if loading}<p role="status">正在加载页面…</p>{:else if error}<p role="alert">{error}</p>{:else}{#if !embedded}<h1 class="mb-6 text-3xl font-bold">{title}</h1>{/if}<div class="prose custom-md max-w-none dark:prose-invert">{@html html}</div>{/if}</article>
