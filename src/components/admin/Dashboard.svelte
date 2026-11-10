<script lang="ts">
import { onMount } from "svelte";
import { apiFetch } from "@/utils/api-client";
import { adminButton as btn } from "./ui";
export let onnavigate: (
	section: "articles" | "users" | "images" | "visits",
) => void;
interface Count {
	pv: number;
	uv: number;
}
interface Overview {
	today: Count;
	yesterday: Count;
	total: Count;
	trend: { date: string; pv: number; uv: number }[];
	top_pages: { path: string; pv: number; uv: number }[];
	recent: {
		id: number;
		path: string;
		title: string;
		ip_address: string;
		occurred_at: string;
	}[];
	content: {
		documents: number;
		users: number;
		comments: number;
		images: number;
		backgrounds: number;
	};
	first_event_at: string | null;
}
let data: Overview | null = null;
let days = 14;
let busy = false;
let error = "";
let chosen = -1;
$: maximum = Math.max(1, ...(data?.trend.map((d) => d.pv) || []));
async function load() {
	busy = true;
	error = "";
	try {
		data = await apiFetch(`/admin/dashboard?days=${days}`);
		chosen = -1;
	} catch (e) {
		error = e instanceof Error ? e.message : "加载失败";
	} finally {
		busy = false;
	}
}
function delta(now: number, previous: number): string {
	if (now === previous && previous > 0) return "与昨日持平";
	return previous
		? `${now >= previous ? "↑" : "↓"} ${Math.abs(((now - previous) / previous) * 100).toFixed(1)}% 较昨日`
		: "昨日暂无访问";
}
function time(value: string): string {
	return new Date(`${value.replace(" ", "T")}Z`).toLocaleString("zh-CN", {
		timeZone: "Asia/Shanghai",
		hour12: false,
	});
}
onMount(() => {
	void load();
});
</script>
<div class="mb-5 flex flex-wrap items-center justify-between gap-3"><p class="text-sm text-slate-500">访问数据按北京时间统计，从本功能上线后开始累计。</p><button class={btn} disabled={busy} onclick={load}>{busy?"刷新中…":"↻ 刷新数据"}</button></div>
{#if error}<p role="alert" class="mb-5 rounded-2xl bg-red-50 p-4 text-red-700">{error}</p>{/if}
{#if data}
<div class="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">{#each [{name:"今日浏览量 · PV",value:data.today.pv,note:delta(data.today.pv,data.yesterday.pv),color:"from-[#85cdca]/20"},{name:"今日访客数 · UV",value:data.today.uv,note:delta(data.today.uv,data.yesterday.uv),color:"from-[#e8a87c]/20"},{name:"累计浏览量 · PV",value:data.total.pv,note:"记录的全部页面访问",color:"from-[#4a6fa5]/15"},{name:"累计访客数 · UV",value:data.total.uv,note:"按浏览器访客标识去重",color:"from-[#c38d94]/20"}] as card}<section class={`rounded-3xl border border-[#4a6fa5]/15 bg-linear-to-br ${card.color} to-white/80 p-6 shadow-sm`}><p class="text-sm text-slate-600">{card.name}</p><p class="my-4 text-4xl text-[#3d5b80]">{card.value.toLocaleString()}</p><p class="text-xs text-slate-500">{card.note}</p></section>{/each}</div>
<div class="my-5 grid gap-4 sm:grid-cols-3 xl:grid-cols-5">{#each [{name:"文档",value:data.content.documents,to:"articles"},{name:"用户",value:data.content.users,to:"users"},{name:"评论",value:data.content.comments,to:null},{name:"图片",value:data.content.images,to:"images"},{name:"背景",value:data.content.backgrounds,to:null}] as item}<div class="flex items-center justify-between rounded-2xl border border-slate-200 bg-white/60 p-4 text-sm">{#if item.to}<button class="rounded-xl px-2 py-1 text-[#4a6fa5] hover:bg-[#85cdca]/20" onclick={()=>onnavigate(item.to as "articles"|"users"|"images")}>{item.name} ↗</button>{:else}<span>{item.name}</span>{/if}<span class="text-xl">{item.value}</span></div>{/each}</div>
<section class="rounded-3xl border border-slate-200 bg-white/85 p-5 md:p-7"><div class="mb-5 flex flex-wrap items-center justify-between gap-3"><h2 class="text-xl">访问趋势</h2><div class="flex gap-2">{#each [7,14,30] as range}<button class={`${btn} ${days===range?"bg-[#85cdca]/25":""}`} disabled={busy} onclick={()=>{days=range;void load();}}>近 {range} 天</button>{/each}</div></div><p class="mb-4 text-sm text-slate-500">{chosen>=0&&data.trend[chosen]?`${data.trend[chosen].date} · 浏览 ${data.trend[chosen].pv} · 访客 ${data.trend[chosen].uv}`:"选择日期查看 PV 与 UV"}</p><div class="flex h-48 items-end gap-1 border-b border-slate-200">{#each data.trend as day,index}<button aria-label={`${day.date}，浏览${day.pv}，访客${day.uv}`} title={`${day.date} · PV ${day.pv} / UV ${day.uv}`} class="group flex h-full min-w-0 flex-1 items-end justify-center gap-0.5 rounded-t-xl px-0.5 hover:bg-[#85cdca]/10" onclick={()=>chosen=index}><span class="w-2/5 rounded-t-lg bg-[#4a6fa5]/65 transition-all duration-500 group-hover:bg-[#4a6fa5]" style={`height:${Math.max(2,day.pv/maximum*100)}%`}></span><span class="w-2/5 rounded-t-lg bg-[#85cdca] transition-all duration-500 group-hover:bg-[#609f9c]" style={`height:${Math.max(2,day.uv/maximum*100)}%`}></span></button>{/each}</div><div class="mt-3 flex justify-between text-xs text-slate-500"><span>{data.trend[0]?.date}</span><span>蓝灰 PV · 青绿 UV</span><span>{data.trend.at(-1)?.date}</span></div>{#if !data.total.pv}<p class="mt-5 text-center text-sm text-slate-500">尚无访问数据，公开页面的访问会自动记录。</p>{/if}</section>
<div class="mt-5 grid gap-5 xl:grid-cols-2"><section class="rounded-3xl border border-slate-200 bg-white/85 p-6"><h2 class="mb-5 text-xl">热门页面 · 近 {days} 天</h2><div class="overflow-x-auto"><table class="w-full text-left text-sm"><thead class="text-slate-500"><tr><th class="pb-3">页面路径</th><th>PV</th><th>UV</th></tr></thead><tbody>{#each data.top_pages as row}<tr class="border-t border-slate-100"><td class="max-w-48 truncate py-4 pr-3" title={row.path}>{row.path}</td><td class="pr-4">{row.pv}</td><td>{row.uv}</td></tr>{/each}</tbody></table></div>{#if !data.top_pages.length}<p class="p-8 text-center text-sm text-slate-500">暂无热门页面</p>{/if}</section><section class="rounded-3xl border border-slate-200 bg-white/85 p-6"><div class="mb-5 flex items-center justify-between gap-3"><h2 class="text-xl">最近访问</h2><button class={btn} onclick={()=>onnavigate("visits")}>全部记录 ↗</button></div>{#each data.recent as row}<div class="border-t border-slate-100 py-3"><p class="truncate text-sm" title={row.title||row.path}>{row.title||row.path}</p><p class="mt-1 text-xs text-slate-500">{row.ip_address} · {time(row.occurred_at)}</p></div>{/each}{#if !data.recent.length}<p class="p-8 text-center text-sm text-slate-500">暂无最近访问</p>{/if}</section></div>
{:else if busy}<p class="p-16 text-center text-slate-500">正在汇总站点数据…</p>{/if}
