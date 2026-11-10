<script lang="ts">
import { onMount } from "svelte";
import { adminButton, adminPrimary, adminField } from "./ui";
import { apiFetch } from "@/utils/api-client";
import { getBackgroundImages } from "@/utils/layout-utils";
interface Bg {
	id: number;
	url: string;
	poster_url: string;
	media_type: string;
	theme: string;
	device: string;
	sort_order: number;
}
let items: Bg[] = [];
let loadedItems: Bg[] = [];
let editingId: number | null = null;
let formDirty = false;
let dropTarget: number | null = null;
let theme = "";
let device = "";
let selected: number[] = [];
let busy = false;
let error = "";
let message = "";
let adding = false;
let deleting = false;
let form = {
	url: "",
	poster_url: "",
	media_type: "image",
	theme: "all",
	device: "desktop",
};
let library: { id: number; url: string; name: string }[] = [];
let picker = false;
let dragged: number | null = null;
let orderChanged = false;
const btn = adminButton;
const field = adminField;
$: filtered = items.filter(
	(i) => (!theme || i.theme === theme) && (!device || i.device === device),
);
$: canSort = filtered.length > 1;
$: canImport = !!theme && !!device;
async function run(fn: () => Promise<void>) {
	if (busy) return;
	busy = true;
	error = "";
	message = "";
	try {
		await fn();
	} catch (e) {
		error = e instanceof Error ? e.message : "操作失败";
	} finally {
		busy = false;
	}
}
async function load() {
	items = await apiFetch<Bg[]>("/backgrounds");
	loadedItems = [...items];
	selected = selected.filter((id) => items.some((item) => item.id === id));
	orderChanged = false;
}
async function importExisting() {
	await run(async () => {
		const config = getBackgroundImages();
		const values = [
			...new Set(config[device === "mobile" ? "mobile" : "desktop"]),
		].filter((url) => url.startsWith("/") || url.startsWith("https://"));
		for (const url of values) {
			if (
				items.some(
					(i) =>
						i.url === url &&
						i.theme === (theme || "light") &&
						i.device === (device || "desktop"),
				)
			)
				continue;
			for (let retry = 0; ; retry++) {
				try {
					await apiFetch("/backgrounds", {
						method: "POST",
						body: JSON.stringify({
							url,
							poster_url: "",
							media_type: "image",
							theme: theme || "light",
							device: device || "desktop",
						}),
					});
					break;
				} catch (e) {
					if (
						!(e instanceof Error) ||
						!("status" in e) ||
						e.status !== 429 ||
						retry >= 30
					)
						throw e;
					await new Promise((resolve) => setTimeout(resolve, 1500));
				}
			}
		}
		await load();
		message = "当前分组的本站壁纸已导入";
	});
}
function discardOrder(): boolean {
	if (orderChanged && !confirm("有未保存的排序，确定放弃？")) return false;
	if (orderChanged) items = [...loadedItems];
	orderChanged = false;
	dragged = null;
	dropTarget = null;
	return true;
}
function changeFilter(key: "theme" | "device", value: string): void {
	if (!discardOrder()) return;
	if (key === "theme") theme = value;
	else device = value;
	selected = [];
}
function openForm(bg?: Bg): void {
	if (!discardOrder()) return;
	editingId = bg?.id ?? null;
	form = bg
		? {
				url: bg.url,
				poster_url: bg.poster_url,
				media_type: bg.media_type,
				theme: bg.theme,
				device: bg.device,
			}
		: {
				url: "",
				poster_url: "",
				media_type: "image",
				theme: theme || "all",
				device: device || "desktop",
			};
	formDirty = false;
	error = "";
	adding = true;
}
function closeForm(): void {
	if (formDirty && !confirm("有未保存的背景修改，确定关闭？")) return;
	adding = false;
	formDirty = false;
	picker = false;
}
export function canLeave(): boolean {
	return (
		!(orderChanged || formDirty) ||
		confirm("有未保存的背景修改或排序，确定离开？")
	);
}
function applyOrder(ids: number[]): void {
	const selectedIds = new Set(ids);
	const rows = new Map(items.map((row) => [row.id, row]));
	let index = 0;
	items = items.map((row) =>
		selectedIds.has(row.id) ? (rows.get(ids[index++]) ?? row) : row,
	);
	orderChanged = items.some((row, i) => row.id !== loadedItems[i]?.id);
}
function drop(id: number): void {
	if (busy || !canSort || dragged === null || dragged === id) return;
	const ids = filtered.map((row) => row.id);
	const from = ids.indexOf(dragged);
	const to = ids.indexOf(id);
	if (from < 0 || to < 0) return;
	ids.splice(from, 1);
	ids.splice(to, 0, dragged);
	applyOrder(ids);
	dragged = null;
	dropTarget = null;
}
function move(id: number, direction: number): void {
	if (busy) return;
	const ids = filtered.map((row) => row.id);
	const index = ids.indexOf(id);
	const target = index + direction;
	if (index < 0 || target < 0 || target >= ids.length) return;
	[ids[index], ids[target]] = [ids[target], ids[index]];
	applyOrder(ids);
}
async function saveForm(): Promise<void> {
	const wasEditing = editingId !== null;
	await run(async () => {
		await apiFetch(wasEditing ? `/backgrounds/${editingId}` : "/backgrounds", {
			method: wasEditing ? "PATCH" : "POST",
			body: JSON.stringify(form),
		});
		adding = false;
		formDirty = false;
		await load();
		message = wasEditing ? "背景已更新" : "背景已添加";
	});
}
async function saveOrder(): Promise<void> {
	await run(async () => {
		await apiFetch("/backgrounds/reorder", {
			method: "POST",
			body: JSON.stringify({ ids: filtered.map((row) => row.id) }),
		});
		await load();
		message = "顺序已保存，刷新前台后按此顺序轮换";
	});
}
function dragStart(event: DragEvent, id: number): void {
	if (!canSort || busy) {
		event.preventDefault();
		return;
	}
	dragged = id;
	if (event.dataTransfer) {
		event.dataTransfer.effectAllowed = "move";
		event.dataTransfer.setData("text/plain", String(id));
	}
}
async function upload(event: Event) {
	const input = event.currentTarget as HTMLInputElement;
	const file = input.files?.[0];
	if (!file) return;
	await run(async () => {
		const data = new FormData();
		data.append("file", file);
		const result = await apiFetch<{ url: string }>(
			form.media_type === "video" ? "/admin/videos" : "/admin/images",
			{ method: "POST", body: data },
		);
		form.url = result.url;
		formDirty = true;
	});
	input.value = "";
}
onMount(() => {
	void run(load);
	const before = (event: BeforeUnloadEvent) => {
		if (orderChanged || formDirty) {
			event.preventDefault();
			event.returnValue = "";
		}
	};
	window.addEventListener("beforeunload", before);
	return () => window.removeEventListener("beforeunload", before);
});
</script>
{#if error && !adding}<p role="alert" class="mb-4 rounded-3xl bg-red-50 p-4 text-red-700">{error}</p>{/if}
{#if message}<p role="status" class="mb-4 rounded-3xl bg-teal-50 p-4 text-teal-800">{message}</p>{/if}
<div class="mb-5 flex flex-wrap justify-between gap-3">
 <div class="flex flex-wrap gap-3">
  <select class={btn} aria-label="主题筛选" value={theme} disabled={busy} onchange={e=>{changeFilter("theme",e.currentTarget.value);e.currentTarget.value=theme;}}><option value="">全部主题</option><option value="all">不限主题</option><option value="light">亮色</option><option value="dark">暗色</option></select>
  <select class={btn} aria-label="设备筛选" value={device} disabled={busy} onchange={e=>{changeFilter("device",e.currentTarget.value);e.currentTarget.value=device;}}><option value="">全部设备</option><option value="desktop">桌面</option><option value="mobile">移动</option></select>
  <span class="self-center text-sm text-slate-500">共 {filtered.length} 项</span>
 </div>
 <button class={adminPrimary} disabled={busy} onclick={()=>openForm()}>＋ 添加背景图</button>
</div>
<div class="mb-4 flex flex-wrap items-center gap-3 text-sm text-slate-500"><button class={btn} disabled={!canImport||busy} onclick={()=>{if(discardOrder())void importExisting();}}>导入本站现有壁纸</button><span>选择主题和设备后，可导入原配置中的图片；“不限主题”同时用于亮色和暗色。</span></div>
<section class="rounded-3xl border border-slate-200 bg-white shadow-[0_4px_20px_rgba(74,111,165,0.07)]">
 <div class="flex flex-wrap items-center gap-3 border-b border-slate-100 p-4 text-sm"><span>已选择 {selected.length} 项</span><span role="status" class={orderChanged?"text-amber-800":"text-slate-500"}>{orderChanged?"排序已修改，记得保存":"拖动卡片调整顺序，也可使用上移、下移按钮"}</span><button class={`${btn} ml-auto`} disabled={!orderChanged||busy} onclick={saveOrder}>保存顺序</button><button class={`${btn} text-red-600`} disabled={!selected.length||busy} onclick={()=>{if(discardOrder())deleting=true;}}>批量移除</button></div>
 <div class="grid gap-5 p-5 sm:grid-cols-2 xl:grid-cols-3">
 {#each filtered as bg,index (bg.id)}
  <article draggable={canSort&&!busy} ondragstart={e=>dragStart(e,bg.id)} ondragover={e=>{if(dragged!==null&&!busy){e.preventDefault();dropTarget=bg.id;if(e.dataTransfer)e.dataTransfer.dropEffect="move";}}} ondrop={e=>{e.preventDefault();drop(bg.id);}} ondragend={()=>{dragged=null;dropTarget=null;}} class={`relative overflow-hidden rounded-3xl border transition-all duration-500 ${dragged===bg.id?"opacity-50":""} ${dropTarget===bg.id&&dragged!==bg.id?"border-[#4a6fa5] bg-[#85cdca]/20 shadow-[0_8px_32px_rgba(74,111,165,0.22)]":"border-slate-200"}`}>
   {#if bg.media_type==="video"}<video draggable="false" src={bg.url} poster={bg.poster_url} muted controls playsinline class="aspect-video w-full bg-slate-100 object-cover"><track kind="captions"/></video>{:else}<img draggable="false" src={bg.url} alt={`${bg.theme==="all"?"不限主题":bg.theme==="dark"?"暗色":"亮色"}背景`} class="aspect-video w-full object-cover"/>{/if}
   <label class="absolute left-3 top-3 rounded-2xl bg-[#fffdfa]/90 p-2"><input type="checkbox" value={bg.id} bind:group={selected} disabled={busy} aria-label={`选择背景 ${bg.id}`}/></label>
   {#if canSort}<span aria-hidden="true" class="absolute right-3 top-3 cursor-grab rounded-2xl bg-[#fffdfa]/90 px-3 py-2 text-slate-700 active:cursor-grabbing">⠿ 拖动</span>{/if}
   <div class="space-y-3 p-4 text-sm"><p>{bg.theme==="all"?"不限主题":bg.theme==="dark"?"暗色":"亮色"} · {bg.device==="desktop"?"桌面":"移动"} · {bg.media_type==="video"?"视频":"图片"}</p><div class="flex flex-wrap gap-2"><button class={`${btn} px-3 py-2`} disabled={busy} onclick={()=>openForm(bg)}>编辑</button><button class={`${btn} px-3 py-2`} aria-label={`上移背景 ${bg.id}`} title="上移" disabled={busy||index===0} onclick={()=>move(bg.id,-1)}>↑</button><button class={`${btn} px-3 py-2`} aria-label={`下移背景 ${bg.id}`} title="下移" disabled={busy||index===filtered.length-1} onclick={()=>move(bg.id,1)}>↓</button><button class={`${btn} ml-auto px-3 py-2 text-red-600`} disabled={busy} onclick={()=>{if(discardOrder()){selected=[bg.id];deleting=true;}}}>移除</button></div></div>
  </article>
 {/each}
 </div>
 {#if !filtered.length}<p class="p-16 text-center text-slate-500">暂无背景图。添加后会按主题和设备应用到博客背景。</p>{/if}
</section>
{#if adding}
 <div class="fixed inset-0 z-50 grid place-items-center bg-slate-900/40 p-5"><form class="max-h-[90vh] w-full max-w-xl space-y-5 overflow-auto rounded-3xl bg-[#fffdfa] p-6" oninput={()=>formDirty=true} onsubmit={e=>{e.preventDefault();void saveForm();}}>
 <div class="flex justify-between"><h2 class="text-lg font-medium">{editingId===null?"添加背景图":"编辑背景图"}</h2><button type="button" class={btn} disabled={busy} onclick={closeForm}>关闭 ×</button></div>
 <fieldset disabled={busy} class="space-y-5">
  <div class="grid grid-cols-2 gap-4"><label>主题<select class={`${field} mt-2`} bind:value={form.theme}><option value="all">不限主题</option><option value="light">亮色</option><option value="dark">暗色</option></select></label><label>设备<select class={`${field} mt-2`} bind:value={form.device}><option value="desktop">桌面</option><option value="mobile">移动</option></select></label></div>
  <p class="text-sm text-slate-500">不限主题的背景会同时参与亮色、暗色模式的轮换。</p>
  <label class="block">媒体类型<select class={`${field} mt-2`} bind:value={form.media_type} onchange={()=>{form.url="";formDirty=true;}}><option value="image">图片</option><option value="video">视频</option></select></label>
  <div class="flex flex-wrap gap-3"><label class={btn}>上传{form.media_type==="video"?"视频（50MiB）":"图片（8MiB）"}<input class="sr-only" type="file" accept={form.media_type==="video"?"video/mp4,video/webm":"image/*"} onchange={upload}/></label>{#if form.media_type==="image"}<button type="button" class={btn} onclick={()=>run(async()=>{library=await apiFetch("/admin/images");picker=true;})}>从已上传图片选择</button>{/if}</div>
  <label class="block">媒体地址<input class={`${field} mt-2`} bind:value={form.url} required placeholder="站内路径或 https://…"/></label>
  {#if form.media_type==="video"}<label class="block">视频封面地址<input class={`${field} mt-2`} bind:value={form.poster_url}/></label><video src={form.url} poster={form.poster_url} controls muted class="max-h-44 w-full"><track kind="captions"/></video>{:else if form.url}<img class="max-h-44 w-full rounded-3xl object-contain" src={form.url} alt="背景预览"/>{/if}
 </fieldset>
 <button class={`${adminPrimary} w-full`} disabled={busy}>{busy?"保存中…":editingId===null?"确认添加":"保存修改"}</button>{#if error}<p role="alert" class="text-red-600">{error}</p>{/if}
 </form></div>
{/if}
{#if picker}<div class="fixed inset-0 z-60 grid place-items-center bg-slate-900/40 p-5"><section class="max-h-[80vh] max-w-3xl overflow-auto rounded-3xl bg-[#fffdfa] p-5"><button class={`${btn} mb-4`} onclick={()=>picker=false}>关闭选图</button><div class="grid grid-cols-2 gap-3 sm:grid-cols-3">{#each library as img}<button onclick={()=>{form.url=img.url;formDirty=true;picker=false;}}><img src={img.url} alt={img.name} class="aspect-video w-full rounded-2xl object-cover"/></button>{/each}</div>{#if !library.length}<p class="p-8">图库为空，请先上传图片。</p>{/if}</section></div>{/if}
{#if deleting}<div class="fixed inset-0 z-50 grid place-items-center bg-slate-900/40 p-5"><section class="rounded-3xl bg-[#fffdfa] p-6"><h2 class="font-normal">移除选中的 {selected.length} 项背景？</h2><p class="my-4 text-sm text-slate-500">移除背景配置后，图库素材仍可继续使用。</p><button class={btn} disabled={busy} onclick={()=>deleting=false}>取消</button><button class={`${btn} ml-3 text-red-600`} disabled={busy} onclick={()=>run(async()=>{await apiFetch("/backgrounds/batch-delete",{method:"POST",body:JSON.stringify({ids:selected})});selected=[];deleting=false;await load();})}>确认移除</button></section></div>{/if}
