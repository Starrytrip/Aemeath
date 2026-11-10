<script lang="ts">
import { onMount } from "svelte";
import { apiFetch } from "@/utils/api-client";
import { adminButton as btn, adminPrimary, adminField as field } from "./ui";
export let viewerId: number;
interface User {
	id: number;
	username: string;
	email: string | null;
	nickname: string | null;
	avatar_url: string | null;
	is_admin: boolean;
	is_active: boolean;
	last_login_at: string | null;
	created_at: string;
}
let rows: User[] = [];
let total = 0;
let page = 1;
let size = 20;
let q = "";
let role = "";
let active = "";
let busy = false;
let error = "";
let message = "";
let sequence = 0;
let editing: User | null = null;
let creating = false;
let form = {
	username: "",
	nickname: "",
	email: "",
	is_admin: false,
	is_active: true,
	password: "",
};
let dirty = false;
let action: {
	kind: "reset-password" | "revoke-sessions" | "delete";
	user: User;
} | null = null;
let password = "";
let confirmPassword = "";
function time(value: string | null): string {
	return value
		? new Date(`${value.replace(" ", "T")}Z`).toLocaleString("zh-CN", {
				timeZone: "Asia/Shanghai",
				hour12: false,
			})
		: "从未登录";
}
async function load(reset = false) {
	if (reset) page = 1;
	const id = ++sequence;
	busy = true;
	error = "";
	try {
		const params = new URLSearchParams({
			page: String(page),
			page_size: String(size),
			q,
			role,
		});
		if (active) params.set("active", active);
		const data = await apiFetch<{ items: User[]; total: number }>(
			`/admin/users?${params}`,
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
function open(user?: User) {
	editing = user || null;
	creating = !user;
	form = {
		username: user?.username || "",
		nickname: user?.nickname || "",
		email: user?.email || "",
		is_admin: user?.is_admin || false,
		is_active: user?.is_active ?? true,
		password: "",
	};
	dirty = false;
	error = "";
}
function close() {
	if (!dirty || confirm("用户资料尚未保存，确定关闭？")) {
		editing = null;
		creating = false;
		dirty = false;
		form.password = "";
	}
}
export function canLeave(): boolean {
	return !dirty || confirm("用户资料尚未保存，确定离开？");
}
async function save() {
	if (
		form.is_admin &&
		!editing?.is_admin &&
		!confirm("确定授予该账号完整后台管理权限？")
	)
		return;
	await run(async () => {
		await apiFetch(editing ? `/admin/users/${editing.id}` : "/admin/users", {
			method: editing ? "PATCH" : "POST",
			body: JSON.stringify(form),
		});
		editing = null;
		creating = false;
		dirty = false;
		form.password = "";
		await load();
		message = "用户资料已保存；权限或状态变化后该用户需重新登录";
	});
}
async function execute() {
	const target = action;
	if (!target) return;
	await run(async () => {
		if (target.kind === "reset-password" && password !== confirmPassword)
			throw Error("两次输入的密码不一致");
		await apiFetch(
			target.kind === "delete"
				? `/admin/users/${target.user.id}`
				: `/admin/users/${target.user.id}/${target.kind}`,
			{
				method: target.kind === "delete" ? "DELETE" : "POST",
				...(target.kind === "reset-password"
					? { body: JSON.stringify({ password }) }
					: {}),
			},
		);
		action = null;
		password = "";
		confirmPassword = "";
		await load();
		message =
			target.kind === "delete"
				? "用户已删除，评论内容已保留"
				: "操作成功，原登录令牌和刷新会话已失效";
	});
}
onMount(() => {
	void load();
	const before = (event: BeforeUnloadEvent) => {
		if (dirty) {
			event.preventDefault();
			event.returnValue = "";
		}
	};
	window.addEventListener("beforeunload", before);
	return () => window.removeEventListener("beforeunload", before);
});
</script>
{#if error&&!editing&&!creating&&!action}<p role="alert" class="mb-4 rounded-2xl bg-red-50 p-4 text-red-700">{error}</p>{/if}{#if message}<p role="status" class="mb-4 rounded-2xl bg-teal-50 p-4 text-teal-800">{message}</p>{/if}
<form class="mb-5 flex flex-wrap items-center gap-3" onsubmit={e=>{e.preventDefault();void load(true);}}><input aria-label="搜索用户" class={`${field} max-w-sm`} placeholder="搜索用户名、邮箱或昵称…" bind:value={q}/><select class={btn} aria-label="角色筛选" bind:value={role}><option value="">全部角色</option><option value="admin">管理员</option><option value="member">普通用户</option></select><select class={btn} aria-label="状态筛选" bind:value={active}><option value="">全部状态</option><option value="true">正常</option><option value="false">禁用</option></select><button class={btn} disabled={busy}>查询</button><button class={`${adminPrimary} ml-auto`} type="button" disabled={busy} onclick={()=>open()}>＋ 新建用户</button></form>
<section class="overflow-hidden rounded-3xl border border-slate-200 bg-white/90 shadow-sm"><div class="flex items-center justify-between gap-3 p-5 text-sm"><span class="text-slate-500">共 {total} 位用户</span><button class={btn} disabled={busy} onclick={()=>load()}>刷新</button></div><div class="overflow-x-auto"><table class="w-full min-w-240 text-left text-sm"><thead class="bg-slate-50 text-slate-500"><tr><th class="p-4">用户</th><th>邮箱</th><th>角色 / 状态</th><th>注册时间</th><th>最近登录</th><th>操作</th></tr></thead><tbody>{#each rows as user}<tr class="border-t border-slate-100"><td class="p-4"><div class="flex items-center gap-3">{#if user.avatar_url}<img class="h-10 w-10 rounded-full object-cover" src={user.avatar_url} alt=""/>{:else}<span class="grid h-10 w-10 place-items-center rounded-full bg-[#85cdca]/20 text-[#4a6fa5]">{(user.nickname||user.username).slice(0,1)}</span>{/if}<div><p>{user.nickname||user.username}{user.id===viewerId?"（我）":""}</p><p class="mt-1 text-xs text-slate-500">@{user.username} · #{user.id}</p></div></div></td><td class="max-w-48 truncate pr-4" title={user.email||""}>{user.email||"未绑定"}</td><td class="space-y-1 pr-4"><p>{user.is_admin?"管理员":"普通用户"}</p><p class={user.is_active?"text-teal-700":"text-red-600"}>{user.is_active?"正常":"已禁用"}</p></td><td class="whitespace-nowrap pr-4">{time(user.created_at)}</td><td class="whitespace-nowrap pr-4">{time(user.last_login_at)}</td><td><div class="flex flex-wrap gap-2 py-3"><button class="rounded-xl px-2 py-1 text-[#4a6fa5] hover:bg-[#85cdca]/20" disabled={busy} onclick={()=>open(user)}>编辑</button><button class="rounded-xl px-2 py-1 hover:bg-[#85cdca]/20 disabled:opacity-40" disabled={busy||user.id===viewerId} onclick={()=>{password="";confirmPassword="";error="";action={kind:"reset-password",user};}}>重置密码</button><button class="rounded-xl px-2 py-1 hover:bg-[#85cdca]/20 disabled:opacity-40" disabled={busy||user.id===viewerId} onclick={()=>{error="";action={kind:"revoke-sessions",user};}}>撤销会话</button><button class="rounded-xl px-2 py-1 text-red-600 hover:bg-[#c38d94]/15 disabled:opacity-40" disabled={busy||user.id===viewerId} onclick={()=>{error="";action={kind:"delete",user};}}>删除</button></div></td></tr>{/each}</tbody></table></div>{#if !rows.length}<p class="p-12 text-center text-slate-500">{busy?"正在加载…":"没有匹配的用户"}</p>{/if}<footer class="flex flex-wrap items-center justify-between gap-3 border-t border-slate-100 p-5 text-sm"><span>第 {page} 页 · 共 {total} 位用户</span><div class="flex gap-2"><button class={btn} disabled={busy||page===1} onclick={()=>{page--;void load();}}>上一页</button><button class={btn} disabled={busy||page*size>=total} onclick={()=>{page++;void load();}}>下一页</button></div></footer></section>
{#if editing||creating}<div class="fixed inset-0 z-50 grid place-items-center bg-slate-900/40 p-5"><form class="max-h-[90vh] w-full max-w-lg space-y-5 overflow-auto rounded-3xl bg-[#fffdfa] p-6" oninput={()=>dirty=true} onsubmit={e=>{e.preventDefault();void save();}}><div class="flex items-center justify-between"><h2 class="text-xl">{creating?"新建用户":"编辑用户"}</h2><button class={btn} type="button" disabled={busy} onclick={close}>关闭 ×</button></div><fieldset class="space-y-4" disabled={busy}><label class="block text-sm">用户名<input class={`${field} mt-2`} bind:value={form.username} readonly={!creating} required minlength="2" maxlength="30"/></label><label class="block text-sm">昵称<input class={`${field} mt-2`} bind:value={form.nickname} maxlength="20"/></label><label class="block text-sm">邮箱<input type="email" class={`${field} mt-2`} bind:value={form.email}/></label>{#if creating}<label class="block text-sm">初始密码<input type="password" class={`${field} mt-2`} bind:value={form.password} minlength="8" maxlength="128" autocomplete="new-password" required/></label>{/if}<div class="flex gap-6"><label class="flex items-center gap-2"><input type="checkbox" bind:checked={form.is_admin} disabled={editing?.id===viewerId}/>管理员</label><label class="flex items-center gap-2"><input type="checkbox" bind:checked={form.is_active} disabled={editing?.id===viewerId}/>启用账号</label></div>{#if editing?.id===viewerId}<p class="text-sm text-slate-500">不能禁用自己或取消自己的管理员身份。</p>{/if}</fieldset>{#if error}<p role="alert" class="text-sm text-red-600">{error}</p>{/if}<button class={`${adminPrimary} w-full`} disabled={busy}>{busy?"保存中…":"保存用户"}</button></form></div>{/if}
{#if action}<div class="fixed inset-0 z-50 grid place-items-center bg-slate-900/40 p-5"><form class="w-full max-w-lg space-y-5 rounded-3xl bg-[#fffdfa] p-6" onsubmit={e=>{e.preventDefault();void execute();}}><h2 class="text-xl">{action.kind==="delete"?"删除用户":action.kind==="reset-password"?"重置密码":"撤销全部登录会话"} · {action.user.username}</h2><p class="text-sm leading-7 text-slate-500">{action.kind==="delete"?"用户账号将被永久删除，原有评论会保留并解除账号关联。":action.kind==="reset-password"?"重置后，该用户的现有登录令牌和刷新会话将失效。":"现有登录令牌和刷新会话都会失效，用户需要重新登录。"}</p>{#if action.kind==="reset-password"}<label class="block text-sm">新密码<input class={`${field} mt-2`} type="password" bind:value={password} minlength="8" maxlength="128" autocomplete="new-password" required/></label><label class="block text-sm">确认新密码<input class={`${field} mt-2`} type="password" bind:value={confirmPassword} minlength="8" maxlength="128" autocomplete="new-password" required/></label>{/if}{#if error}<p role="alert" class="text-red-600">{error}</p>{/if}<div class="flex justify-end gap-3"><button type="button" class={btn} disabled={busy} onclick={()=>{action=null;password="";confirmPassword="";}}>取消</button><button class={adminPrimary} disabled={busy}>{busy?"处理中…":"确认操作"}</button></div></form></div>{/if}
