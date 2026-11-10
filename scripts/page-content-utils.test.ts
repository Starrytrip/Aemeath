import { test } from "node:test";
import assert from "node:assert/strict";
import { cleanPageMarkdown } from "./page-content-utils";
test("legacy decoration is removed while text, links, images and Markdown remain", async () => {
	const input =
		'# 关于我\n\n你好 <span class="profile-author-trigger">Rain<span class="profile-author-tooltip"><img src="/tooltip.png"/>隐藏说明</span></span>，欢迎。\n\n<span class="profile-about-accent">重点</span> [文章](/reader/)\n\n<img src="/api/v1/uploads/a.png" alt="照片"/>\n';
	const result = await cleanPageMarkdown(input);
	assert.equal(
		result,
		"# 关于我\n\n你好 Rain，欢迎。\n\n重点 [文章](/reader/)\n\n![照片](</api/v1/uploads/a.png>)\n",
	);
	assert.equal(await cleanPageMarkdown(result), result);
});
test("HTML examples in inline code and fences are preserved verbatim", async () => {
	const input =
		'Use `<span class="demo">Hi</span>`.\n\n```html\n<img src="/code.png"/>\n```\n\n**粗体** & 正文\n';
	assert.equal(await cleanPageMarkdown(input), input);
});

test("repository directives become normal editable links", async () => {
	const input = '::github{repo="Jarvis0227/Yukina"}\n';
	const result = await cleanPageMarkdown(input);
	assert.equal(
		result,
		"[Jarvis0227/Yukina](https://github.com/Jarvis0227/Yukina)\n",
	);
	assert.equal(await cleanPageMarkdown(result), result);
});

test("legacy MDX friend template becomes readable text without executing code", async () => {
	const input =
		'export const site = {name:"测试博客",url:"https://example.com"};\nexport const notes = [{title:"原则",content:"保持友善"}];\n<div>{site.name}</div>';
	const result = await cleanPageMarkdown(input);
	assert.match(result, /站点名称：测试博客/);
	assert.match(result, /保持友善/);
	assert.ok(!result.includes("export const"));
	assert.equal(await cleanPageMarkdown(result), result);
});
