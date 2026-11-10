<script lang="ts">
import { onMount } from "svelte";
import Vditor from "vditor";
import "vditor/dist/index.css";
import { apiFetch } from "@/utils/api-client";
export let value = "";
export let mode: "ir" | "wysiwyg" = "ir";
export let disabled = false;
export let onchange: (text: string) => void;
export let onlibrary: () => void;
let host: HTMLDivElement;
let editor: Vditor;
let ready = false;
let disposed = false;
export function getValue(): string {
	return ready ? editor.getValue() : value;
}
$: if (editor && ready) {
	if (disabled) editor.disabled();
	else editor.enable();
}
export function insert(text: string) {
	editor?.insertValue(text);
}
onMount(() => {
	editor = new Vditor(host, {
		cdn: "/vendor/vditor",
		height: 560,
		mode,
		value,
		cache: { enable: false },
		toolbar: [
			"emoji",
			"headings",
			"bold",
			"italic",
			"strike",
			"|",
			"line",
			"quote",
			"list",
			"ordered-list",
			"check",
			"|",
			"code",
			"inline-code",
			"link",
			"upload",
			{ name: "library", tip: "从图库选择", icon: "▧", click: onlibrary },
			"table",
			"|",
			"undo",
			"redo",
			"|",
			"edit-mode",
			"fullscreen",
			"outline",
			"preview",
		],
		input: onchange,
		upload: {
			accept: "image/*",
			max: 8 * 1024 * 1024,
			handler: async (files: File[]) => {
				try {
					for (const file of files) {
						const form = new FormData();
						form.append("file", file);
						const saved = await apiFetch<{ url: string }>("/admin/images", {
							method: "POST",
							body: form,
						});
						editor.insertValue(`![](${saved.url})\n`);
					}
					return null;
				} catch (e) {
					return e instanceof Error ? e.message : "上传失败";
				}
			},
		},
		after: () => {
			if (disposed) {
				editor.destroy();
				return;
			}
			editor.setValue(value);
			ready = true;
			if (disabled) editor.disabled();
		},
		preview: { markdown: { sanitize: true } },
	});
	return () => {
		disposed = true;
		if (ready) editor.destroy();
		ready = false;
	};
});
</script>
<div bind:this={host}></div>
