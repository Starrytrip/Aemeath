<script lang="ts">
import { onMount } from "svelte";
import { apiFetch } from "@/utils/api-client";
import {
	adminButton as button,
	adminPrimary as primary,
	adminField as field,
} from "./ui";
interface Song {
	id: string;
	name: string;
	artist: string;
	url: string;
	cover: string;
	lrc: string;
}
interface Configuration {
	mode: string;
	volume: number;
	play_mode: string;
	show_lyrics: boolean;
	songs: Song[];
	meting: {
		api: string;
		server: string;
		type: string;
		id: string;
		fallback_apis: string[];
	};
}
interface Resolved {
	playlist: { name: string; artist: string; url: string; pic: string }[];
	warning: string | null;
}
let config: Configuration | null = null;
let revision = 0;
let configured = false;
let hasAuth = false;
let auth = "";
let clearAuth = false;
let snapshot = "";
let fallbackText = "";
let busy = false;
let error = "";
let message = "";
let draft: Song | null = null;
let originalSong = "";
let dragId = "";
let dropId = "";
let preview: Resolved | null = null;
let images: { id: number; name: string; url: string }[] = [];
let pickingCover = false;
const card =
	"rounded-3xl border border-slate-200 bg-white/85 p-5 shadow-sm md:p-7";
$: dirty =
	!!config && (JSON.stringify(config) !== snapshot || !!auth || clearAuth);
export function canLeave() {
	return (
		!(dirty || (draft && JSON.stringify(draft) !== originalSong)) ||
		confirm("音乐配置尚未保存，确定离开吗？")
	);
}
function beforeUnload(event: BeforeUnloadEvent) {
	if (dirty || draft) {
		event.preventDefault();
		event.returnValue = "";
	}
}
async function run(action: () => Promise<void>) {
	if (busy) return;
	busy = true;
	error = "";
	message = "";
	try {
		await action();
	} catch (e) {
		error = e instanceof Error ? e.message : "操作失败";
	} finally {
		busy = false;
	}
}
async function load() {
	const data = await apiFetch<{
		configuration: Configuration;
		revision: number;
		has_auth: boolean;
		configured: boolean;
	}>("/admin/music");
	config = data.configuration;
	fallbackText = config.meting.fallback_apis.join("\n");
	revision = data.revision;
	hasAuth = data.has_auth;
	configured = data.configured;
	auth = "";
	clearAuth = false;
	snapshot = JSON.stringify(config);
	preview = null;
}
async function save() {
	await run(async () => {
		if (!config) return;
		const result = await apiFetch<{ revision: number }>("/admin/music", {
			method: "PUT",
			body: JSON.stringify({
				configuration: config,
				revision,
				auth: auth || null,
				clear_auth: clearAuth,
			}),
		});
		revision = result.revision;
		hasAuth = clearAuth ? false : !!auth || hasAuth;
		auth = "";
		clearAuth = false;
		configured = true;
		snapshot = JSON.stringify(config);
		preview = null;
		message = "音乐配置已保存，刷新前台页面后生效。";
	});
}
function edit(song?: Song) {
	draft = song
		? { ...song }
		: {
				id: crypto.randomUUID(),
				name: "",
				artist: "",
				url: "",
				cover: "",
				lrc: "",
			};
	originalSong = JSON.stringify(draft);
	pickingCover = false;
}
function closeDraft() {
	if (
		draft &&
		JSON.stringify(draft) !== originalSong &&
		!confirm("放弃这首歌曲的修改吗？")
	)
		return;
	draft = null;
	pickingCover = false;
}
function applySong() {
	if (!config || !draft) return;
	if (!draft.name.trim() || !draft.url.trim()) {
		error = "请填写歌曲名称和音频地址";
		return;
	}
	const index = config.songs.findIndex((s) => s.id === draft?.id);
	config.songs =
		index < 0
			? [...config.songs, { ...draft }]
			: config.songs.map((s) => (s.id === draft?.id ? { ...draft! } : s));
	config = { ...config };
	draft = null;
	error = "";
}
function move(id: string, target: number) {
	if (!config || target < 0 || target >= config.songs.length) return;
	const songs = [...config.songs];
	const index = songs.findIndex((s) => s.id === id);
	if (index < 0) return;
	const [song] = songs.splice(index, 1);
	songs.splice(target, 0, song);
	config.songs = songs;
	config = { ...config };
}
function startDrag(event: PointerEvent, id: string) {
	if (busy || event.button !== 0) return;
	event.preventDefault();
	dragId = id;
	dropId = id;
	(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}
function updateDrag(event: PointerEvent) {
	if (!dragId) return;
	const row = document
		.elementFromPoint(event.clientX, event.clientY)
		?.closest<HTMLElement>("[data-song-id]");
	if (row?.dataset.songId) dropId = row.dataset.songId;
}
function finishDrag() {
	if (config && dragId && dropId)
		move(
			dragId,
			config.songs.findIndex((s) => s.id === dropId),
		);
	dragId = "";
	dropId = "";
}
function remove(song: Song) {
	if (config && confirm(`从歌单移除“${song.name}”吗？已上传的文件会保留。`)) {
		config.songs = config.songs.filter((s) => s.id !== song.id);
		config = { ...config };
	}
}
async function upload(event: Event, kind: "audio" | "lyrics") {
	const input = event.currentTarget as HTMLInputElement;
	const file = input.files?.[0];
	if (!file || !draft) return;
	await run(async () => {
		if (file.size > (kind === "audio" ? 50 * 1024 * 1024 : 512 * 1024))
			throw new Error(
				kind === "audio" ? "音频不能超过 50 MiB" : "歌词不能超过 512 KiB",
			);
		const form = new FormData();
		form.append("file", file);
		const result = await apiFetch<{ url: string }>("/admin/music/upload", {
			method: "POST",
			body: form,
		});
		if (draft) {
			if (kind === "audio") {
				draft.url = result.url;
				if (!draft.name) draft.name = file.name.replace(/\.[^.]+$/, "");
			} else draft.lrc = result.url;
			draft = { ...draft };
		}
	});
	input.value = "";
}
async function chooseCover() {
	await run(async () => {
		images = await apiFetch<typeof images>("/admin/images");
		pickingCover = true;
	});
}
onMount(() => {
	void run(load);
	window.addEventListener("beforeunload", beforeUnload);
	return () => window.removeEventListener("beforeunload", beforeUnload);
});
</script>

<div class="space-y-6">
 <div class="flex flex-wrap items-center justify-between gap-3"><p class="text-sm text-slate-500">{configured ? "正在使用后台音乐配置" : "尚未保存，前台沿用项目配置"}{dirty ? " · 有未保存修改" : ""}</p><div class="flex flex-wrap gap-2"><button class={button} disabled={busy} onclick={()=>{if(canLeave()) void run(load);}}>重新加载</button><button class={primary} disabled={busy || !config} onclick={save}>{busy ? "处理中…" : "保存音乐配置"}</button></div></div>
 {#if error}<p role="alert" class="rounded-2xl border border-red-200 bg-red-50 p-4 text-sm text-red-700">{error}</p>{/if}
 {#if message}<p role="status" class="rounded-2xl bg-teal-50 p-4 text-sm text-teal-800">{message}</p>{/if}
 {#if config}
 <section class={card}>
  <h2 class="text-lg text-slate-800">播放设置</h2><p class="mt-2 text-sm leading-6 text-slate-500">切换模式会保留另一种模式的配置。访客已保存的音量优先于这里的默认值。</p>
  <div class="mt-5 grid gap-4 sm:grid-cols-2"><button class={`${config.mode === "local" ? "border-teal-300 bg-teal-50 shadow-sm" : "border-slate-200 hover:border-teal-300 hover:bg-teal-50/40"} rounded-2xl border p-5 text-left transition-all hover:-translate-y-0.5`} disabled={busy} aria-pressed={config.mode === "local"} onclick={()=>{if(config) config.mode="local";}}><span class="text-base text-slate-800">♫ 本地歌单</span><span class="mt-2 block text-sm text-slate-500">上传音频或使用链接，自由编辑与排序</span></button><button class={`${config.mode === "meting" ? "border-teal-300 bg-teal-50 shadow-sm" : "border-slate-200 hover:border-teal-300 hover:bg-teal-50/40"} rounded-2xl border p-5 text-left transition-all hover:-translate-y-0.5`} disabled={busy} aria-pressed={config.mode === "meting"} onclick={()=>{if(config) config.mode="meting";}}><span class="text-base text-slate-800">☁ Meting 歌单</span><span class="mt-2 block text-sm text-slate-500">通过音乐平台 ID 自动解析，支持备用接口</span></button></div>
  <div class="mt-6 grid items-end gap-5 sm:grid-cols-3"><label class="space-y-2 text-sm text-slate-600">默认音量 · {Math.round(config.volume*100)}%<input aria-label="默认音量" class="block w-full accent-teal-600" type="range" min="0" max="1" step="0.01" bind:value={config.volume} disabled={busy}/></label><label class="space-y-2 text-sm text-slate-600">播放方式<select class={field} bind:value={config.play_mode} disabled={busy}><option value="list">列表循环</option><option value="one">单曲循环</option><option value="random">随机播放</option></select></label><label class="flex min-h-11 items-center gap-3 text-sm text-slate-600"><input class="h-4 w-4 accent-teal-600" type="checkbox" bind:checked={config.show_lyrics} disabled={busy}/>显示歌词</label></div>
 </section>
 {#if config.mode === "local"}
 <section class={card}><div class="flex flex-wrap items-center justify-between gap-3"><div><h2 class="text-lg text-slate-800">我的歌单 <span class="text-sm text-slate-400">{config.songs.length} 首</span></h2><p class="mt-2 text-sm text-slate-500">拖动手柄排序，也可使用上下按钮。编辑完成后统一保存。</p></div><button class={primary} disabled={busy || config.songs.length>=500} onclick={()=>edit()}>＋ 添加歌曲</button></div>
 <ol class="mt-5 space-y-3">{#each config.songs as song,index (song.id)}<li data-song-id={song.id} class={`flex flex-wrap items-center gap-4 rounded-2xl border p-4 transition-all hover:border-teal-200 hover:bg-teal-50/40 ${dragId===song.id ? "border-teal-300 opacity-50" : dropId===song.id && dragId ? "border-teal-400 bg-teal-50" : "border-slate-100 bg-slate-50/50"}`}><button class="touch-none select-none cursor-grab rounded-lg px-2 py-3 text-xl text-slate-400 hover:bg-white hover:text-teal-700 active:cursor-grabbing" disabled={busy} aria-label={`拖动排序 ${song.name}`} onpointerdown={event=>startDrag(event,song.id)} onpointermove={updateDrag} onpointerup={finishDrag} onpointercancel={()=>{dragId="";dropId="";}}>⠿</button>{#if song.cover}<img src={song.cover} alt="" class="h-12 w-12 rounded-xl object-cover" loading="lazy"/>{:else}<span class="grid h-12 w-12 place-items-center rounded-xl bg-teal-100 text-xl text-teal-700">♪</span>{/if}<div class="min-w-0 flex-1"><p class="truncate text-sm text-slate-800">{index+1}. {song.name}</p><p class="mt-1 truncate text-xs text-slate-500">{song.artist || "未知歌手"}</p></div><div class="flex flex-wrap gap-2"><button class={button} disabled={busy || index===0} aria-label={`上移 ${song.name}`} onclick={()=>move(song.id,index-1)}>↑</button><button class={button} disabled={busy || index===config!.songs.length-1} aria-label={`下移 ${song.name}`} onclick={()=>move(song.id,index+1)}>↓</button><button class={button} disabled={busy} onclick={()=>edit(song)}>编辑</button><button class={`${button} text-red-600`} disabled={busy} onclick={()=>remove(song)}>移除</button></div></li>{/each}</ol>
 {#if !config.songs.length}<div class="py-14 text-center text-sm text-slate-400">还没有歌曲，添加一首喜欢的音乐吧。</div>{/if}</section>
 {:else}
 <section class={card}><h2 class="text-lg text-slate-800">Meting 解析设置</h2><p class="mt-2 text-sm leading-6 text-slate-500">由后端请求主接口，失败后依次使用备用接口。解析结果缓存 5 分钟。</p><div class="mt-5 grid gap-5 sm:grid-cols-2"><label class="space-y-2 text-sm text-slate-600">音乐平台<select class={field} disabled={busy} bind:value={config.meting.server}><option value="netease">网易云音乐</option><option value="tencent">QQ 音乐</option><option value="kugou">酷狗音乐</option><option value="baidu">百度音乐</option><option value="xiami">虾米音乐</option></select></label><label class="space-y-2 text-sm text-slate-600">解析类型<select class={field} disabled={busy} bind:value={config.meting.type}><option value="playlist">歌单</option><option value="song">单曲</option><option value="album">专辑</option><option value="artist">歌手</option><option value="search">搜索</option></select></label><label class="space-y-2 text-sm text-slate-600 sm:col-span-2">ID / 搜索关键词<input class={field} disabled={busy} bind:value={config.meting.id} placeholder="填写对应的歌单或歌曲 ID"/></label><label class="space-y-2 text-sm text-slate-600 sm:col-span-2">主接口模板<input class={field} disabled={busy} bind:value={config.meting.api}/><span class="block text-xs leading-6 text-slate-400">支持 :server、:type、:id、:r 占位符，接口需使用公网 HTTPS 地址。</span></label><label class="space-y-2 text-sm text-slate-600 sm:col-span-2">备用接口（每行一个，最多 4 个）<textarea class={`${field} min-h-28 font-mono text-xs`} disabled={busy} bind:value={fallbackText} oninput={event=>{if(config) config.meting.fallback_apis=event.currentTarget.value.split("\n").map(s=>s.trim()).filter(Boolean);}}></textarea></label><label class="space-y-2 text-sm text-slate-600 sm:col-span-2">认证信息（可选）<input class={field} type="password" autocomplete="new-password" disabled={busy || clearAuth} bind:value={auth} placeholder={hasAuth ? "已设置；留空保留原认证信息" : "仅填写接口需要的 auth 值"}/><span class="block text-xs text-slate-400">只保存在后端，不会返回给前台。请勿把密钥直接写入接口模板。</span></label>{#if hasAuth}<label class="flex items-center gap-3 text-sm text-slate-600"><input type="checkbox" disabled={busy} bind:checked={clearAuth}/>清除已保存的认证信息</label>{/if}</div></section>
 {/if}
 <section class={card}><div class="flex flex-wrap items-center justify-between gap-3"><div><h2 class="text-lg text-slate-800">解析与试听</h2><p class="mt-2 text-sm text-slate-500">保存后可以查看前台实际获取的歌单。试听由你主动播放。</p></div><button class={button} disabled={busy || dirty || !configured} onclick={()=>run(async()=>{preview=await apiFetch<Resolved>("/admin/music/preview",{method:"POST"});})}>刷新解析 / 试听</button></div>{#if dirty}<p class="mt-4 text-xs text-amber-700">请先保存当前修改再试听。</p>{/if}{#if preview}{#if preview.warning}<p role="alert" class="mt-5 rounded-xl bg-amber-50 p-4 text-sm text-amber-800">{preview.warning}</p>{/if}<p class="mt-4 text-sm text-slate-500">已解析 {preview.playlist.length} 首</p><div class="mt-4 max-h-96 space-y-3 overflow-y-auto">{#each preview.playlist as song}<div class="flex flex-wrap items-center justify-between gap-3 rounded-xl bg-slate-50 p-3"><div class="min-w-0 flex-1 text-sm"><p class="truncate text-slate-800">{song.name}</p><p class="text-xs text-slate-500">{song.artist}</p></div><audio controls preload="none" src={song.url} class="h-10 max-w-full" aria-label={`试听 ${song.name}`}></audio></div>{/each}</div>{/if}</section>
 {/if}
</div>

{#if draft}
<div class="fixed inset-0 z-50 grid place-items-center overflow-y-auto bg-slate-900/30 p-4 backdrop-blur-sm" role="presentation"><div role="dialog" aria-modal="true" aria-labelledby="music-song-title" tabindex="-1" class="my-auto max-h-[90vh] w-full max-w-2xl overflow-y-auto rounded-3xl border border-white/80 bg-[#fffdfa] p-6 shadow-2xl"><div class="flex items-center justify-between"><h2 id="music-song-title" class="text-xl text-slate-800">编辑歌曲</h2><button class={button} disabled={busy} onclick={closeDraft}>关闭</button></div><div class="mt-6 space-y-5"><div class="grid gap-4 sm:grid-cols-2"><label class="space-y-2 text-sm text-slate-600">歌曲名称<input class={field} disabled={busy} bind:value={draft.name}/></label><label class="space-y-2 text-sm text-slate-600">歌手<input class={field} disabled={busy} bind:value={draft.artist}/></label></div><label class="block space-y-2 text-sm text-slate-600">音频地址<input class={field} disabled={busy} bind:value={draft.url} placeholder="https://… 或 /music/song.mp3"/></label><label class={`${button} inline-flex cursor-pointer`}>上传音频（最大 50 MiB）<input class="sr-only" type="file" disabled={busy} accept=".mp3,.wav,.flac,.ogg,.m4a" onchange={event=>upload(event,"audio")}/></label><label class="block space-y-2 text-sm text-slate-600">封面地址<input class={field} disabled={busy} bind:value={draft.cover} placeholder="可选，HTTPS 或站内地址"/></label><button class={button} disabled={busy} onclick={chooseCover}>从图片素材选择</button>{#if draft.cover}<img src={draft.cover} alt="封面预览" class="h-24 w-24 rounded-2xl object-cover"/>{/if}{#if pickingCover}<div class="grid max-h-48 grid-cols-4 gap-2 overflow-y-auto rounded-xl bg-slate-50 p-3">{#each images as image}<button class="overflow-hidden rounded-lg border border-transparent hover:border-teal-400 hover:shadow-md" title={image.name} onclick={()=>{if(draft) draft.cover=image.url;pickingCover=false;}}><img src={image.url} alt={image.name} class="aspect-square w-full object-cover"/></button>{/each}{#if !images.length}<p class="col-span-4 text-sm text-slate-500">图片素材中还没有图片，请先上传。</p>{/if}</div>{/if}<label class="block space-y-2 text-sm text-slate-600">歌词地址 / LRC 文本<textarea class={`${field} min-h-28 font-mono text-xs`} disabled={busy} bind:value={draft.lrc} placeholder="可选，HTTPS / 站内地址或 [00:00.00] 开头的歌词"></textarea></label><label class={`${button} inline-flex cursor-pointer`}>上传 LRC 歌词<input type="file" class="sr-only" disabled={busy} accept=".lrc" onchange={event=>upload(event,"lyrics")}/></label><p class="text-xs leading-6 text-slate-400">支持 MP3、WAV、FLAC、OGG、M4A；具体播放能力取决于浏览器。上传文件会保存在后端，移除歌曲后文件保留。</p>{#if error}<p role="alert" class="text-sm text-red-600">{error}</p>{/if}<div class="flex justify-end gap-3"><button class={button} disabled={busy} onclick={closeDraft}>取消</button><button class={primary} disabled={busy} onclick={applySong}>完成编辑</button></div></div></div></div>
{/if}
