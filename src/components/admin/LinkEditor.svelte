<script lang="ts">
import LinkEditor from "./LinkEditor.svelte";
import { getAvailableIcons } from "@/constants/icons";
import type { ManagedLink } from "@/types/managed-site";
import { adminButton as button, adminField as field } from "./ui";
export let rows: ManagedLink[] = [];
export let nested = false;
export let onchange: () => void = () => {};
function move(i: number, d: number) {
	const next = [...rows];
	[next[i], next[i + d]] = [next[i + d], next[i]];
	rows = next;
	onchange();
}
</script>
<div class="space-y-3">{#each rows as row,i}<div class="space-y-3 rounded-xl border border-slate-200 p-3"><div class="grid gap-3 sm:grid-cols-2"><label class="text-sm">名称<input class={`${field} mt-1`} bind:value={row.name} oninput={onchange}/></label><label class="text-sm">链接<input class={`${field} mt-1`} bind:value={row.url} oninput={onchange}/></label><label class="text-sm">图标<select class={`${field} mt-1`} bind:value={row.icon} onchange={onchange}><option value="">默认图标</option>{#each getAvailableIcons().sort() as icon}<option value={icon}>{icon}</option>{/each}</select></label><div class="flex items-center gap-3"><label class="text-sm"><input type="checkbox" checked={row.enabled!==false} onchange={e=>{row.enabled=e.currentTarget.checked;onchange();}}/> 显示</label><label class="text-sm"><input type="checkbox" bind:checked={row.external} onchange={onchange}/> 新窗口</label><button type="button" class={button} disabled={i===0} onclick={()=>move(i,-1)} aria-label="上移">↑</button><button type="button" class={button} disabled={i===rows.length-1} onclick={()=>move(i,1)} aria-label="下移">↓</button><button type="button" class="text-sm text-red-600" onclick={()=>{rows=rows.filter((_,n)=>n!==i);onchange();}}>移除</button></div></div>{#if nested}<div class="border-l-2 border-cyan-100 pl-3"><LinkEditor bind:rows={row.children} onchange={onchange}/></div>{/if}</div>{/each}<button type="button" class={button} onclick={()=>{rows=[...rows,{name:"新链接",url:"/",enabled:true,children:nested?[]:undefined}];onchange();}}>＋ 添加{nested?"菜单":"链接"}</button></div>
