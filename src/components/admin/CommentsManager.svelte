<script lang="ts">
import { onMount } from "svelte";
import { apiFetch } from "@/utils/api-client";
import { adminButton as button, adminPrimary, adminField as field } from "./ui";
interface Row {
	id: number;
	page: string;
	nick: string;
	content: string;
	parent_id: number | null;
	created_at: string;
}
let rows: Row[] = [];
let total = 0;
let page = 1;
let query = "";
let path = "";
let busy = false;
let error = "";
let target: Row | null = null;
let reply = "";
export function canLeave() {
	return !reply.trim() || confirm("回复尚未发送，确定离开？");
}
async function load() {
	busy = true;
	error = "";
	try {
		const r = await apiFetch<{ items: Row[]; total: number }>(
			`/admin/comments?${new URLSearchParams({ page: String(page), q: query, page_path: path })}`,
		);
		rows = r.items;
		total = r.total;
	} catch (e) {
		error = e instanceof Error ? e.message : "加载失败";
	} finally {
		busy = false;
	}
}
async function remove(r: Row) {
	if (!confirm("删除这条评论及其回复？")) return;
	busy = true;
	error = "";
	try {
		await apiFetch(`/comments/${r.id}`, { method: "DELETE" });
		await load();
	} catch (e) {
		error = e instanceof Error ? e.message : "删除失败";
	} finally {
		busy = false;
	}
}
async function send() {
	if (!target || !reply.trim()) return;
	busy = true;
	error = "";
	try {
		await apiFetch("/comments", {
			method: "POST",
			body: JSON.stringify({
				page: target.page,
				parent_id: target.id,
				content: reply,
			}),
		});
		target = null;
		reply = "";
		await load();
	} catch (e) {
		error = e instanceof Error ? e.message : "回复失败";
	} finally {
		busy = false;
	}
}
onMount(() => {
	void load();
	const before = (e: BeforeUnloadEvent) => {
		if (reply.trim()) {
			e.preventDefault();
			e.returnValue = "";
		}
	};
	window.addEventListener("beforeunload", before);
	return () => window.removeEventListener("beforeunload", before);
});
</script>
<form class="mb-5 flex flex-wrap gap-3" onsubmit={e=>{e.preventDefault();page=1;void load();}}><input aria-label="搜索评论" class={`${field} max-w-sm`} bind:value={query} placeholder="搜索评论内容或昵称"/><input aria-label="所属页面" class={`${field} max-w-sm`} bind:value={path} placeholder="所属页面路径（可留空）"/><button class={adminPrimary} disabled={busy}>筛选</button></form>
{#if error}<p role="alert" class="mb-4 text-red-600">{error}</p>{/if}
<div class="space-y-4">{#each rows as r}<article class="rounded-3xl border border-slate-200 bg-white/90 p-5"><header class="flex flex-wrap items-center gap-3 text-sm"><strong>{r.nick}</strong><time class="text-slate-400">{r.created_at}</time><a class="text-[#4a6fa5]" href={r.page} target="_blank" rel="noreferrer">查看所属页面 ↗</a>{#if r.parent_id}<span class="text-slate-400">回复 #{r.parent_id}</span>{/if}</header><p class="my-4 whitespace-pre-wrap break-words text-sm leading-7">{r.content}</p><div class="flex gap-4 text-sm"><button class="text-[#4a6fa5]" disabled={busy} onclick={()=>{if(canLeave()){target=r;reply="";}}}>回复</button><button class="text-red-600" disabled={busy} onclick={()=>remove(r)}>删除</button></div>{#if target?.id===r.id}<form class="mt-4 space-y-3" onsubmit={e=>{e.preventDefault();void send();}}><textarea aria-label={`回复 ${r.nick}`} class={field} bind:value={reply} maxlength="1000" required></textarea><div class="flex gap-3"><button class={adminPrimary} disabled={busy}>发送回复</button><button type="button" class={button} onclick={()=>{if(canLeave()){target=null;reply="";}}}>取消</button></div></form>{/if}</article>{/each}{#if !rows.length}<p class="rounded-3xl bg-white p-12 text-center text-slate-500">{busy?"正在加载…":"没有匹配的评论"}</p>{/if}</div>
<div class="mt-5 flex items-center justify-center gap-4"><button class={button} disabled={busy||page===1} onclick={()=>{page--;void load();}}>上一页</button><span class="text-sm">第 {page} 页 · 共 {total} 条</span><button class={button} disabled={busy||page*30>=total} onclick={()=>{page++;void load();}}>下一页</button></div>
