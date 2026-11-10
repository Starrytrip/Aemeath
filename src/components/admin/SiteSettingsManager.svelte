<script lang="ts">
import { onMount } from "svelte";
import { apiFetch } from "@/utils/api-client";
import { siteDefaults } from "@/utils/site-defaults";
import type { ManagedSite } from "@/types/managed-site";
import { adminButton as button, adminPrimary, adminField as field } from "./ui";
import LinkEditor from "./LinkEditor.svelte";
import ImageField from "./ImageField.svelte";
let form: ManagedSite = structuredClone(siteDefaults);
let revision = 0;
let busy = false;
let dirty = false;
let error = "";
let message = "";
let tab = "basic";
const tabs = [
	{ key: "basic", name: "基础信息" },
	{ key: "profile", name: "博主资料" },
	{ key: "announcement", name: "公告" },
	{ key: "navigation", name: "导航菜单" },
	{ key: "appearance", name: "外观默认值" },
	{ key: "sidebar", name: "侧栏布局" },
	{ key: "footer", name: "页脚" },
];
const names: Record<string, string> = {
	profile: "博主名片",
	announcement: "公告",
	categories: "分类",
	tags: "标签",
	sidebarToc: "文章目录",
	stats: "站点统计",
	calendar: "日历",
	music: "音乐",
	siteInfo: "站点信息",
	timeGreeting: "时钟问候",
	quoteOfTheDay: "今日一言",
	scheduleProgress: "时间进度",
};
export function canLeave() {
	return !dirty || confirm("站点设置尚未保存，确定离开？");
}
function changed() {
	dirty = true;
	message = "";
}
async function load() {
	if (!canLeave()) return;
	busy = true;
	error = "";
	try {
		const r = await apiFetch<{ data: ManagedSite | null; revision: number }>(
			"/admin/site/settings",
		);
		form = structuredClone(r.data || siteDefaults);
		form.navigation.forEach((l) => (l.children ||= []));
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
	message = "";
	try {
		const r = await apiFetch<{ revision: number }>("/admin/site/settings", {
			method: "PUT",
			body: JSON.stringify({ data: form, revision }),
		});
		revision = r.revision;
		dirty = false;
		message =
			"设置已保存，前台刷新或切换页面后生效。已有访客的个人外观选择会保留。";
		window.dispatchEvent(new Event("yukina:settings-updated"));
	} catch (e) {
		error = e instanceof Error ? e.message : "保存失败";
	} finally {
		busy = false;
	}
}
function move(i: number, d: number) {
	const rows = [...form.sidebar.widgets];
	[rows[i], rows[i + d]] = [rows[i + d], rows[i]];
	form.sidebar.widgets = rows;
	changed();
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
<form onsubmit={e=>{e.preventDefault();void save();}} oninput={changed}>
<div class="mb-5 flex flex-wrap items-center justify-between gap-3"><p class="text-sm text-slate-500">{dirty?"有未保存的修改":"站点公共配置"}</p><div class="flex gap-2"><button type="button" class={button} disabled={busy} onclick={load}>重新加载</button><button class={adminPrimary} disabled={busy}>{busy?"处理中…":"保存设置"}</button></div></div>
{#if error}<p role="alert" class="mb-4 rounded-xl bg-red-50 p-4 text-red-700">{error}</p>{/if}{#if message}<p role="status" class="mb-4 rounded-xl bg-emerald-50 p-4 text-emerald-700">{message}</p>{/if}
<div class="mb-5 flex flex-wrap gap-2">{#each tabs as t}<button type="button" class={tab===t.key?adminPrimary:button} onclick={()=>tab=t.key}>{t.name}</button>{/each}</div>
<fieldset disabled={busy} class="space-y-5 rounded-3xl border border-slate-200 bg-white/90 p-6">
{#if tab==="basic"}
<div class="grid gap-5 md:grid-cols-2">{#each [["title","网站名称"],["subtitle","网站副标题"],["site_url","网站完整地址"],["start_date","建站日期"],["keywords","关键词（逗号分隔）"]] as [key,label]}<label class="text-sm">{label}<input class={`${field} mt-2`} bind:value={form.basic[key as keyof typeof form.basic]} type={key==="start_date"?"date":"text"} required={key==="title"||key==="site_url"}/></label>{/each}</div>
<label class="block text-sm">网站简介<textarea class={`${field} mt-2 min-h-24`} bind:value={form.basic.description}></textarea></label><div class="grid gap-5 md:grid-cols-2"><ImageField label="网站 Logo" bind:value={form.basic.logo} onchange={changed}/><ImageField label="站点图标" bind:value={form.basic.favicon} onchange={changed}/></div>
{:else if tab==="profile"}
<div class="grid gap-5 md:grid-cols-2"><label class="text-sm">昵称<input class={`${field} mt-2`} bind:value={form.profile.name}/></label><ImageField label="博主头像" bind:value={form.profile.avatar} onchange={changed}/></div><label class="block text-sm">个人介绍<textarea class={`${field} mt-2 min-h-24`} bind:value={form.profile.bio}></textarea></label><h3>社交链接</h3><LinkEditor bind:rows={form.profile.links} onchange={changed}/>
{:else if tab==="announcement"}
<label class="mr-5"><input type="checkbox" bind:checked={form.announcement.enabled}/> 显示公告</label><label><input type="checkbox" bind:checked={form.announcement.closable}/> 允许访客关闭</label><label class="block text-sm">标题<input class={`${field} mt-2`} bind:value={form.announcement.title}/></label><label class="block text-sm">公告正文<textarea class={`${field} mt-2 min-h-32`} bind:value={form.announcement.content}></textarea></label><LinkEditor bind:rows={form.announcement.links} onchange={changed}/>
{:else if tab==="navigation"}<p class="text-sm text-slate-500">调整菜单顺序、显示状态和子菜单。隐藏菜单后，已有页面仍可通过链接访问。</p><LinkEditor bind:rows={form.navigation} nested onchange={changed}/>
{:else if tab==="footer"}<label class="block text-sm">版权文字<input class={`${field} mt-2`} bind:value={form.footer.copyright}/></label><h3>备案与其他链接</h3><LinkEditor bind:rows={form.footer.records} onchange={changed}/>
{:else if tab==="appearance"}
<p class="text-sm text-slate-500">这里设置站点默认值。访客已保存的个人偏好优先。</p>
<div class="grid gap-5 md:grid-cols-2">
<label>默认主题<select class={`${field} mt-2`} bind:value={form.appearance.theme}><option value="yukina">Yukina</option><option value="pure">Pure</option></select></label>
<label>明暗模式<select class={`${field} mt-2`} bind:value={form.appearance.mode}><option value="system">跟随系统</option><option value="light">浅色</option><option value="dark">深色</option></select></label>
<label>文章列表<select class={`${field} mt-2`} bind:value={form.appearance.list_mode}><option value="grid">网格</option><option value="list">列表</option></select></label>
<label>导航栏<select class={`${field} mt-2`} bind:value={form.appearance.navbar_behavior}><option value="auto">下滚收缩，上滚展开</option><option value="static">始终展开</option></select></label>
<label>卡片装饰<select class={`${field} mt-2`} bind:value={form.appearance.card_decoration}><option value="soft">柔和蓝调</option><option value="flat">轻薄边框</option></select></label>
<label>壁纸模式<select class={`${field} mt-2`} bind:value={form.appearance.wallpaper_mode}><option value="banner">横幅壁纸</option><option value="fullscreen">全屏壁纸</option><option value="overlay">全屏透明</option><option value="none">纯色背景</option></select></label>
{#each [["hue","主题色相",0,360,1],["posts_per_page","每页文章数",1,50,1],["sakura_count","樱花数量",0,100,1],["sakura_speed","樱花速度倍数",0.2,3,0.1],["carousel_interval","壁纸轮播间隔（秒）",3,120,1]] as [key,label,min,max,step]}<label>{label}<input class={`${field} mt-2`} type="number" {min} {max} {step} bind:value={form.appearance[key as "hue"|"posts_per_page"|"sakura_count"|"sakura_speed"|"carousel_interval"]}/></label>{/each}
</div><div class="flex flex-wrap gap-5"><label><input type="checkbox" bind:checked={form.appearance.sakura_enabled}/> 默认启用樱花</label><label><input type="checkbox" bind:checked={form.appearance.intro_enabled}/> 默认播放开屏动画</label></div>
{:else if tab==="sidebar"}<p class="text-sm text-slate-500">按列表顺序显示，可调整左右位置及文章页可见性。</p>{#each form.sidebar.widgets as w,i}<div class="flex flex-wrap items-center gap-4 rounded-xl border border-slate-100 p-3"><strong class="w-24 text-sm">{names[w.type]||w.type}</strong><label class="text-sm"><input type="checkbox" bind:checked={w.enabled}/> 显示</label><label class="text-sm">位置 <select class="rounded-lg border p-1" bind:value={w.side}><option value="left">左侧</option><option value="right">右侧</option></select></label><label class="text-sm"><input type="checkbox" bind:checked={w.show_on_post}/> 文章页显示</label><label class="text-sm"><input type="checkbox" bind:checked={w.only_on_post}/> 仅文章页</label><label class="text-sm"><input type="checkbox" bind:checked={w.sticky}/> 跟随滚动</label><button type="button" class={button} disabled={i===0} onclick={()=>move(i,-1)} aria-label="上移组件">↑</button><button type="button" class={button} disabled={i===form.sidebar.widgets.length-1} onclick={()=>move(i,1)} aria-label="下移组件">↓</button></div>{/each}
{/if}
</fieldset></form>
