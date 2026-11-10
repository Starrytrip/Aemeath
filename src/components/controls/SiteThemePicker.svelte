<script lang="ts">
import { onMount } from "svelte";
import { siteThemeConfig } from "@/config/siteThemeConfig";
import { chooseSiteTheme } from "@/utils/site-theme";

let current = siteThemeConfig.defaultTheme;
const choices = [
	{ value: "yukina", name: "Yukina", detail: "壁纸 · 卡片 · 侧栏" },
	{ value: "pure", name: "Pure", detail: "简洁导航 · 专注阅读" },
] as const;

onMount(() => {
	current =
		document.documentElement.dataset.siteTheme === "pure" ? "pure" : "yukina";
});
</script>

<div class="theme-choices" aria-label="选择站点主题">
	{#each choices as choice}
		<button type="button" aria-pressed={current === choice.value} onclick={() => chooseSiteTheme(choice.value)}>
			<span class="theme-name">{choice.name}<span aria-hidden="true">{current === choice.value ? "✓" : ""}</span></span>
			<small>{choice.detail}</small>
		</button>
	{/each}
</div>

<style>
	.theme-choices { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: .5rem; }
	button { min-width: 0; padding: .85rem; text-align: left; border: 1px solid var(--line-divider); border-radius: .75rem; color: var(--content-meta); }
	button:hover { background: var(--btn-regular-bg); }
	button[aria-pressed="true"] { border-color: var(--primary); color: var(--primary); }
	.theme-name { display: flex; justify-content: space-between; gap: .5rem; font-size: .9rem; font-weight: 700; }
	small { display: block; margin-top: .4rem; font-size: .7rem; line-height: 1.6; }
	button:focus-visible { outline: 2px solid var(--primary); outline-offset: 3px; }
</style>
