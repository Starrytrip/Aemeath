<script lang="ts">
import { onMount } from "svelte";
import { siteDefaults } from "@/utils/site-defaults";
import { loadSiteSettings } from "@/utils/managed-site";
import Icon from "@/components/common/Icon.svelte";
export let mobile = false;
let links = siteDefaults.navigation;
let path = "";
let holder: HTMLDivElement;
onMount(() => {
	let active = true;
	path = location.pathname;
	loadSiteSettings()
		.then((c) => {
			if (active && c) links = c.navigation;
		})
		.catch(() => {});
	const sync = () => {
		path = location.pathname;
		if (window.__yukinaSiteDefaults)
			links = window.__yukinaSiteDefaults.navigation;
		holder
			?.querySelectorAll("details")
			.forEach((d) => d.removeAttribute("open"));
	};
	const events = [
		"astro:page-load",
		"swup:page:view",
		"swup:contentReplaced",
		"yukina:site-settings",
	];
	for (const event of events) document.addEventListener(event, sync);
	return () => {
		active = false;
		for (const event of events) document.removeEventListener(event, sync);
	};
});
function selected(url: string) {
	return (
		url !== "#" &&
		(url === "/" ? path === "/" : path.startsWith(url.split("?")[0]))
	);
}
</script>
<div bind:this={holder} class:mobile class="managed-nav">{#each links.filter(l=>l.enabled!==false) as l}
{#if l.children?.some(c=>c.enabled!==false)}<details class="managed-nav-group"><summary class:active={l.children.some(c=>selected(c.url))}><Icon icon={l.icon||"material-symbols:folder"}/><span>{l.name}</span><span class="text-xs">⌄</span></summary><div class="managed-submenu">{#each l.children.filter(c=>c.enabled!==false) as c}<a href={c.url} class:active={selected(c.url)} target={c.external?"_blank":undefined} rel={c.external?"noopener noreferrer":undefined} data-mobile-menu-close><Icon icon={c.icon||"material-symbols:arrow-forward"}/>{c.name}</a>{/each}</div></details>
{:else}<a href={l.url} class:active={selected(l.url)} target={l.external?"_blank":undefined} rel={l.external?"noopener noreferrer":undefined} data-mobile-menu-close><Icon icon={l.icon||"material-symbols:home"}/>{l.name}</a>{/if}
{/each}</div>
<style>
.managed-nav{display:flex;align-items:center;gap:.2rem;font-weight:600;color:var(--content-regular)}.managed-nav>a,.managed-nav summary,.managed-submenu a{display:flex;align-items:center;gap:.4rem;padding:.65rem .8rem;border-radius:2rem;white-space:nowrap;font-size:.95rem}.managed-nav summary{cursor:pointer;list-style:none}.managed-nav summary::-webkit-details-marker{display:none}.active,.managed-nav a:hover,.managed-nav summary:hover{color:var(--primary);background:color-mix(in srgb,var(--primary) 12%,transparent)}.managed-nav-group{position:relative}.managed-submenu{position:absolute;top:100%;left:0;min-width:10rem;border:1px solid var(--line-divider);border-radius:1rem;padding:.5rem;background:var(--card-bg);box-shadow:0 8px 30px #0001;z-index:80}.mobile{display:flex;flex-direction:column;align-items:stretch;gap:.4rem}.mobile .managed-submenu{position:static;box-shadow:none;margin-left:1rem}.mobile a,.mobile summary{padding:1rem;border-radius:1rem}
</style>
