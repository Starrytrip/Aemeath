<script lang="ts">
import { onMount } from "svelte";
import { apiFetch } from "@/utils/api-client";
import { adminButton as btn, adminPrimary, adminField as field } from "./ui";
export interface Visit {
	id: number;
	path: string;
	title: string;
	ip_address: string;
	region?: string;
	referrer: string;
	user_agent: string;
	occurred_at: string;
}
let rows: Visit[] = [];
let total = 0;
let page = 1;
let size = 30;
let ip = "";
let path = "";
let start = "";
let end = "";
let busy = false;
let error = "";
let sequence = 0;
function time(value: string): string {
	return new Date(`${value.replace(" ", "T")}Z`).toLocaleString("zh-CN", {
		timeZone: "Asia/Shanghai",
		hour12: false,
	});
}
async function load(reset = false) {
	if (reset) page = 1;
	const id = ++sequence;
	busy = true;
	error = "";
	try {
		const data = await apiFetch<{ items: Visit[]; total: number }>(
			`/admin/visits?${new URLSearchParams({ page: String(page), page_size: String(size), ip, path, start, end })}`,
		);
		if (id === sequence) {
			rows = data.items;
			total = data.total;
		}
	} catch (e) {
		if (id === sequence) error = e instanceof Error ? e.message : "加载失败";
	} finally {
		if (id === sequence) busy = false;
	}
}
onMount(() => {
	void load();
});
</script>
<form class="mb-5 grid gap-4 rounded-3xl border border-slate-200 bg-white/80 p-5 sm:grid-cols-2 xl:grid-cols-5" onsubmit={e=>{e.preventDefault();void load(true);}}>
 <label class="text-sm">IP 地址<input class={`${field} mt-2`} bind:value={ip} placeholder="精确匹配IP"/></label><label class="text-sm">页面或标题<input class={`${field} mt-2`} bind:value={path} placeholder="筛选页面…"/></label><label class="text-sm">开始日期<input type="date" class={`${field} mt-2`} bind:value={start}/></label><label class="text-sm">结束日期<input type="date" class={`${field} mt-2`} bind:value={end}/></label><div class="flex items-end gap-2"><button class={adminPrimary} disabled={busy}>查询</button><button class={btn} type="button" disabled={busy} onclick={()=>{ip="";path="";start="";end="";void load(true);}}>重置</button></div>
</form>
{#if error}<p role="alert" class="mb-4 rounded-2xl bg-red-50 p-4 text-red-700">{error}</p>{/if}
<section class="overflow-hidden rounded-3xl border border-slate-200 bg-white/90 shadow-sm"><div class="flex flex-wrap items-center justify-between gap-3 p-5"><p class="text-sm text-slate-500">访问记录 · 北京时间 · IP 信息仅管理员可见</p><button class={btn} disabled={busy} onclick={()=>load()}>刷新</button></div><div class="overflow-x-auto"><table class="w-full min-w-250 text-left text-sm"><thead class="bg-slate-50 text-slate-500"><tr><th class="p-4">时间</th><th>IP / 来源地</th><th>事件</th><th>页面</th><th>来源</th><th>浏览器信息</th></tr></thead><tbody>{#each rows as row}<tr class="border-t border-slate-100"><td class="whitespace-nowrap p-4">{time(row.occurred_at)}</td><td class="whitespace-nowrap pr-4">{row.ip_address}<p class="mt-1 text-xs text-slate-500">{row.region||"—"}</p></td><td class="whitespace-nowrap pr-4">页面访问</td><td class="max-w-64 pr-4"><p class="truncate" title={row.title}>{row.title||row.path}</p><p class="mt-1 truncate text-xs text-slate-500" title={row.path}>{row.path}</p></td><td class="max-w-48 truncate pr-4" title={row.referrer}>{row.referrer||"直接访问"}</td><td class="max-w-64 truncate pr-4" title={row.user_agent}>{row.user_agent||"—"}</td></tr>{/each}</tbody></table></div>{#if !rows.length}<p class="p-12 text-center text-slate-500">{busy?"正在加载…":"暂无访问记录，公开页面产生访问后会显示在这里"}</p>{/if}<footer class="flex flex-wrap items-center justify-between gap-3 border-t border-slate-100 p-5 text-sm"><span>共 {total} 条 · 第 {page} 页</span><div class="flex flex-wrap gap-2"><select aria-label="每页条数" class={btn} bind:value={size} onchange={()=>load(true)} disabled={busy}><option value={20}>20条/页</option><option value={30}>30条/页</option><option value={50}>50条/页</option><option value={100}>100条/页</option></select><button class={btn} disabled={busy||page===1} onclick={()=>{page--;void load();}}>上一页</button><button class={btn} disabled={busy||page*size>=total} onclick={()=>{page++;void load();}}>下一页</button></div></footer></section>
