<script lang="ts">
import { onMount } from "svelte";
import { apiFetch } from "@/utils/api-client";
import { friendDefaults } from "@/utils/site-defaults";
import type { ManagedFriend } from "@/types/managed-site";
import ImageField from "./ImageField.svelte";
import { adminButton as button, adminPrimary, adminField as field } from "./ui";
let rows: ManagedFriend[] = [];
let revision = 0;
let dirty = false;
let busy = false;
let error = "";
let message = "";
export function canLeave() {
	return !dirty || confirm("友链有未保存的修改，确定离开？");
}
async function load() {
	if (!canLeave()) return;
	busy = true;
	error = "";
	try {
		const r = await apiFetch<{
			data: ManagedFriend[] | null;
			revision: number;
		}>("/admin/site/friends");
		rows = r.data || structuredClone(friendDefaults);
		revision = r.revision;
		dirty = false;
	} catch (e) {
		error = e instanceof Error ? e.message : "加载失败";
	} finally {
		busy = false;
	}
}
async function save() {
	busy = true;
	error = "";
	try {
		const r = await apiFetch<{ revision: number }>("/admin/site/friends", {
			method: "PUT",
			body: JSON.stringify({ data: rows, revision }),
		});
		revision = r.revision;
		dirty = false;
		message = "友链已保存";
	} catch (e) {
		error = e instanceof Error ? e.message : "保存失败";
	} finally {
		busy = false;
	}
}
function add() {
	rows = [
		...rows,
		{
			id: crypto.randomUUID(),
			title: "",
			description: "",
			siteurl: "",
			imgurl: "",
			rss: "",
			weight: 0,
			enabled: true,
			recommended: false,
		},
	];
	dirty = true;
}
onMount(() => {
	void load();
	const before = (e: BeforeUnloadEvent) => {
		if (dirty) {
			e.preventDefault();
			e.returnValue = "";
		}
	};
	window.addEventListener("beforeunload", before);
	return () => window.removeEventListener("beforeunload", before);
});
</script>
<div class="mb-5 flex flex-wrap justify-between gap-3"><p class="text-sm text-slate-500">共 {rows.length} 个友链{dirty?" · 未保存":""}</p><div class="flex gap-2"><button class={button} disabled={busy} onclick={load}>重新加载</button><button class={button} disabled={busy} onclick={add}>＋ 添加友链</button><button class={adminPrimary} disabled={busy} onclick={save}>保存友链</button></div></div>
{#if error}<p role="alert" class="mb-4 text-red-600">{error}</p>{/if}{#if message}<p role="status" class="mb-4 text-emerald-700">{message}</p>{/if}
<fieldset disabled={busy} class="space-y-5" oninput={()=>dirty=true}>{#each rows as r,i}<section class="space-y-4 rounded-3xl border border-slate-200 bg-white/90 p-6"><div class="flex items-center justify-between"><h2>{r.title||"新友链"}</h2><button class="text-sm text-red-600" onclick={()=>{if(confirm("从列表移除这个友链？保存后生效。")){rows=rows.filter((_,n)=>n!==i);dirty=true;}}}>移除</button></div><div class="grid gap-4 md:grid-cols-2">{#each [["title","网站名称"],["siteurl","网站地址"],["description","介绍"],["rss","RSS 地址"]] as [key,label]}<label class="text-sm">{label}<input class={`${field} mt-2`} bind:value={r[key as "title"|"siteurl"|"description"|"rss"]}/></label>{/each}<ImageField label="头像" bind:value={r.imgurl} onchange={()=>dirty=true}/><label class="text-sm">排序权重（越大越靠前）<input class={`${field} mt-2`} type="number" min="-100000" max="100000" bind:value={r.weight}/></label></div><div class="flex gap-5"><label><input type="checkbox" bind:checked={r.enabled}/> 上架</label><label><input type="checkbox" bind:checked={r.recommended}/> 推荐</label></div></section>{/each}{#if !rows.length}<p class="rounded-3xl bg-white p-12 text-center text-slate-500">还没有友链，点击添加友链。</p>{/if}</fieldset>
