<script lang="ts">
import { apiFetch } from "@/utils/api-client";
import { adminButton as button, adminField as field } from "./ui";
export let value = "";
export let label = "图片";
export let onchange: () => void = () => {};
let rows: { id: number; url: string; name: string }[] = [];
let open = false;
let busy = false;
let error = "";
async function library() {
	try {
		rows = await apiFetch("/admin/images");
		open = true;
	} catch (e) {
		error = e instanceof Error ? e.message : "图库加载失败";
	}
}
async function upload(e: Event) {
	const input = e.currentTarget as HTMLInputElement;
	const file = input.files?.[0];
	if (!file) return;
	busy = true;
	error = "";
	try {
		const form = new FormData();
		form.append("file", file);
		const r = await apiFetch<{ url: string }>("/admin/images/import", {
			method: "POST",
			body: form,
		});
		value = r.url;
		onchange();
	} catch (e) {
		error = e instanceof Error ? e.message : "上传失败";
	} finally {
		busy = false;
		input.value = "";
	}
}
</script>
<div class="space-y-2"><label class="block text-sm">{label}<input class={`${field} mt-2`} bind:value oninput={onchange} placeholder="图片地址或从图库选择" /></label><div class="flex items-center gap-2"><button type="button" class={button} onclick={library}>选择图片</button><label class={`${button} cursor-pointer`}>{busy?"上传中…":"上传图片"}<input class="sr-only" type="file" accept="image/png,image/jpeg,image/webp,image/gif" disabled={busy} onchange={upload}/></label>{#if value}<img src={value} class="h-10 w-16 rounded-lg object-contain" alt={label}/>{/if}</div>{#if error}<p role="alert" class="text-sm text-red-600">{error}</p>{/if}</div>
{#if open}<div class="fixed inset-0 z-50 grid place-items-center bg-black/40 p-5"><div role="dialog" aria-modal="true" aria-label="选择图片" tabindex="-1" class="max-h-[80vh] w-full max-w-3xl overflow-y-auto rounded-2xl bg-white p-5"><div class="mb-4 flex justify-between"><h3>选择图片</h3><button type="button" class={button} onclick={()=>open=false}>关闭</button></div><div class="grid grid-cols-3 gap-3 sm:grid-cols-5">{#each rows as r}<button type="button" title={r.name} class="rounded-xl border p-2 hover:border-cyan-500" onclick={()=>{value=r.url;open=false;onchange();}}><img class="aspect-square w-full object-contain" src={r.url} alt={r.name} loading="lazy"/><span class="block truncate text-xs">{r.name}</span></button>{/each}</div>{#if !rows.length}<p class="p-8 text-center">图库还没有图片</p>{/if}</div></div>{/if}
