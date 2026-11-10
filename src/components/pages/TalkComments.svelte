<script lang="ts">
import { onMount } from "svelte";
import {
	type AuthUser,
	apiFetch,
	type CommentRow,
	fetchMe,
} from "@/utils/api-client";
export let talkId: number;
let items: CommentRow[] = [];
let loading = true;
let busy = false;
let error = "";
let user: AuthUser | null = null;
let nick = "";
let content = "";
let replying: CommentRow | null = null;
$: page = `/talks/${talkId}/`;
async function load() {
	loading = true;
	error = "";
	try {
		items = await apiFetch<CommentRow[]>(
			`/comments?page=${encodeURIComponent(page)}`,
		);
	} catch (e) {
		error = e instanceof Error ? e.message : "评论加载失败";
	} finally {
		loading = false;
	}
}
async function submit() {
	if (busy) return;
	busy = true;
	error = "";
	try {
		const saved = await apiFetch<CommentRow>("/comments", {
			method: "POST",
			body: JSON.stringify({
				page,
				nick: nick.trim(),
				content: content.trim(),
				parent_id: replying?.id || null,
			}),
		});
		items = [...items, saved];
		content = "";
		replying = null;
		try {
			localStorage.setItem("yukina.comment-nick", nick.trim());
		} catch {}
	} catch (e) {
		error = e instanceof Error ? e.message : "评论发送失败";
	} finally {
		busy = false;
	}
}
async function remove(id: number) {
	if (busy || !confirm("删除这条评论？其回复也将隐藏。")) return;
	busy = true;
	error = "";
	try {
		await apiFetch(`/comments/${id}`, { method: "DELETE" });
		await load();
	} catch (e) {
		error = e instanceof Error ? e.message : "删除失败";
	} finally {
		busy = false;
	}
}
function time(value: string) {
	const normalized = value.replace(" ", "T");
	const timestamp = /(?:Z|[+-]\d{2}:\d{2})$/i.test(normalized)
		? normalized
		: `${normalized}Z`;
	return new Date(timestamp).toLocaleString("zh-CN", {
		timeZone: "Asia/Shanghai",
		hour12: false,
	});
}
onMount(() => {
	try {
		nick = localStorage.getItem("yukina.comment-nick") || "";
	} catch {}
	void load();
	void fetchMe()
		.then((value) => {
			user = value;
		})
		.catch(() => {});
});
</script>
<section class="talk-comments" aria-label="说说评论">
 <div class="comments-heading"><h3>聊一聊 <span>{items.length}</span></h3><button type="button" disabled={busy||loading} onclick={load}>刷新评论</button></div>
 {#if loading}<p role="status" class="comment-hint">正在加载评论…</p>{:else if !items.length}<p class="comment-hint">还没有评论，留下第一句问候吧。</p>{:else}<ol class="comment-list">{#each items as item (item.id)}<li class:reply={!!item.parent_id}><div class="comment-meta"><strong>{item.nick}</strong>{#if item.is_admin}<span class="author-label">博主</span>{/if}<time datetime={item.created_at}>{time(item.created_at)}</time></div>{#if item.parent_id}<p class="reply-to">回复 @{items.find(parent=>parent.id===item.parent_id)?.nick||"访客"}</p>{/if}<p class="comment-body">{item.content}</p><div class="comment-actions"><button type="button" disabled={busy} onclick={()=>{replying=item;}}>回复</button>{#if user?.is_admin}<button type="button" disabled={busy} onclick={()=>remove(item.id)}>删除</button>{/if}</div></li>{/each}</ol>{/if}
 {#if error}<p role="alert" class="comment-error">{error}</p>{/if}
 <form onsubmit={e=>{e.preventDefault();void submit();}} class="comment-form">
  <fieldset disabled={busy||loading}>
   {#if user}<p class="comment-hint">以 {user.nickname||user.username} 的身份评论</p>{:else}<label>昵称<input bind:value={nick} required maxlength="50" autocomplete="nickname" placeholder="怎么称呼你？"/></label>{/if}
   {#if replying}<div class="reply-banner">回复 @{replying.nick}<button type="button" onclick={()=>{replying=null;}}>取消回复</button></div>{/if}
   <label><span class="sr-only">评论内容</span><textarea bind:value={content} required maxlength="1000" placeholder="友善交流，分享你的想法…" rows="3"></textarea></label>
   <div class="comment-submit"><span>{content.length} / 1000</span><button class="comment-send" disabled={!content.trim()||(!user&&!nick.trim())}>{busy?"发送中…":"发送评论"}</button></div>
  </fieldset>
 </form>
</section>
<style>
 .talk-comments{margin-top:1.25rem;border-top:1px solid var(--line-divider);padding-top:1.25rem;}
 .comments-heading,.comment-meta,.comment-actions,.comment-submit,.reply-banner{display:flex;align-items:center;gap:.65rem;}
 .comments-heading{justify-content:space-between;}.comments-heading h3{font-weight:600;font-size:.9rem;}.comments-heading h3 span{color:var(--primary);margin-left:.3rem;}
 button{font-size:.75rem;color:var(--primary);border-radius:.6rem;}button:focus-visible{outline:2px solid var(--primary);outline-offset:3px;}button:disabled{opacity:.5;}
 .comment-hint{padding:.8rem 0;font-size:.8rem;color:var(--content-meta);}.comment-list{margin-top:1rem;max-height:26rem;overflow:auto;list-style:none;padding:0;}.comment-list li{padding:1rem 0;border-bottom:1px solid var(--line-divider);}.comment-list li.reply{padding-left:1rem;border-left:2px solid var(--line-divider);}.comment-meta{flex-wrap:wrap;font-size:.8rem;}.comment-meta time{margin-left:auto;font-size:.7rem;color:var(--content-meta);}.author-label{border-radius:99px;padding:.1rem .5rem;background:var(--btn-regular-bg);color:var(--primary);font-size:.65rem;}.comment-body{margin:.55rem 0;white-space:pre-wrap;overflow-wrap:anywhere;font-size:.85rem;line-height:1.8;}.reply-to{color:var(--content-meta);font-size:.7rem;margin-top:.5rem;}.comment-actions{justify-content:flex-end;}.comment-error{color:#c34747;font-size:.8rem;margin-top:.8rem;}.comment-form{margin-top:1rem;}.comment-form fieldset{display:grid;gap:.75rem;}.comment-form label{display:grid;gap:.4rem;font-size:.8rem;}.comment-form input,.comment-form textarea{width:100%;padding:.8rem 1rem;border:1px solid var(--line-divider);border-radius:1rem;background:var(--btn-regular-bg);outline:none;color:inherit;font:inherit;}.comment-form textarea{resize:vertical;line-height:1.8;}.comment-form input:focus,.comment-form textarea:focus{border-color:var(--primary);}.comment-submit{justify-content:space-between;font-size:.7rem;color:var(--content-meta);}.comment-send{padding:.65rem 1.1rem;background:var(--primary);color:var(--page-bg);font-weight:600;}.reply-banner{justify-content:space-between;color:var(--primary);font-size:.8rem;}
</style>
