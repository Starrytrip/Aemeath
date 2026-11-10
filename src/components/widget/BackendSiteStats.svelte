<script lang="ts">
import { onMount } from "svelte";
import {
	loadPublishedArticles,
	articleTaxonomy,
} from "@/utils/published-articles";
import { loadSiteSettings } from "@/utils/managed-site";
let stats: { name: string; value: string | number }[] = [];
let error = "";
onMount(() => {
	let active = true;
	Promise.all([loadPublishedArticles(), loadSiteSettings()])
		.then(([p, s]) => {
			if (active)
				stats = [
					{ name: "文章", value: p.length },
					{ name: "分类", value: articleTaxonomy(p, "category").length },
					{ name: "标签", value: articleTaxonomy(p, "tag").length },
					{
						name: "总字数",
						value: p.reduce((sum, p) => sum + p.words, 0).toLocaleString(),
					},
					{
						name: "运行天数",
						value: Math.max(
							0,
							Math.floor(
								(Date.now() -
									new Date(s?.basic.start_date || "2026-01-01").getTime()) /
									86400000,
							),
						),
					},
				];
		})
		.catch((e) => {
			if (active) error = e.message;
		});
	return () => {
		active = false;
	};
});
</script>
{#if error}<p role="alert" class="text-sm">{error}</p>{:else}<dl class="space-y-3 py-2">{#each stats as s}<div class="flex justify-between gap-3"><dt class="text-sm opacity-70">{s.name}</dt><dd class="font-semibold">{s.value}</dd></div>{/each}</dl>{/if}
