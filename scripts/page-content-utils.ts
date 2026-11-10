import ts from "typescript";
import { createRequire } from "node:module";
import { createMarkdownProcessor } from "@astrojs/markdown-remark";
import type { Root, RootContent } from "mdast";
const require = createRequire(import.meta.url);
const { Parser } = createRequire(require.resolve("sanitize-html"))(
	"htmlparser2",
);

/** Remove presentation HTML while preserving Markdown, code examples and actual images. */
export async function cleanPageMarkdown(markdown: string): Promise<string> {
	const protectedParts: { start: number; end: number; text: string }[] = [];
	const processor = await createMarkdownProcessor({
		remarkPlugins: [
			() => (tree: Root) => {
				function walk(node: Root | RootContent) {
					if (
						[
							"code",
							"inlineCode",
							"image",
							"link",
							"definition",
							"imageReference",
							"linkReference",
						].includes(node.type)
					) {
						const start = node.position?.start.offset,
							end = node.position?.end.offset;
						if (start !== undefined && end !== undefined)
							protectedParts.push({
								start,
								end,
								text: markdown.slice(start, end),
							});
					} else if ("children" in node)
						for (const child of node.children) walk(child as RootContent);
				}
				walk(tree);
				tree.children = [];
			},
		],
	});
	await processor.render(markdown);
	let source = markdown;
	const prefix = `YUKINA_CODE_${Math.random().toString(36).slice(2)}_`;
	for (let i = protectedParts.length - 1; i >= 0; i--) {
		const part = protectedParts[i];
		source =
			source.slice(0, part.start) +
			`${prefix}${i}_END` +
			source.slice(part.end);
	}
	source = source.replace(
		/^::github\{repo=["']([a-zA-Z0-9_.-]+\/[a-zA-Z0-9_.-]+)["']\}\s*$/gm,
		(_, repo: string) => `[${repo}](https://github.com/${repo})\n`,
	);
	const legacy = legacyFriends(source);
	if (legacy !== null) source = legacy;
	let result = "";
	let suppressed = 0;
	const stack: { suppressed: boolean; close: string }[] = [];
	const safeUrl = (url: string) =>
		/^(https?:\/\/|\/(?!\/)|\.\.?\/|#)/.test(url) && !/[\r\n\\]/.test(url);
	const parser = new Parser(
		{
			onopentag(name: string, attrs: Record<string, string>) {
				const hide =
					suppressed > 0 ||
					["script", "style", "iframe"].includes(name) ||
					/(?:^|\s)profile-author-tooltip(?:\s|$)/.test(attrs.class || "");
				let close = "";
				if (hide) suppressed++;
				else if (name === "img" && safeUrl(attrs.src || ""))
					result += `![${(attrs.alt || "").replace(/[\[\]]/g, "")}](<${attrs.src.replaceAll(">", "%3E")}>)`;
				else if (name === "br") result += "\n";
				else if (
					["p", "div", "section", "article", "details", "summary"].includes(
						name,
					)
				) {
					result += "\n";
					close = "\n";
				} else if (/^h[1-6]$/.test(name)) {
					result += `\n${"#".repeat(Number(name[1]))} `;
					close = "\n";
				} else if (name === "strong" || name === "b") {
					result += "**";
					close = "**";
				} else if (name === "em" || name === "i") {
					result += "*";
					close = "*";
				} else if (name === "a" && safeUrl(attrs.href || "")) {
					result += "[";
					close = `](<${attrs.href.replaceAll(">", "%3E")}>)`;
				}
				stack.push({ suppressed: hide, close });
			},
			ontext(text: string) {
				if (!suppressed) result += text;
			},
			onclosetag() {
				const node = stack.pop();
				if (node?.suppressed) suppressed--;
				else if (node) result += node.close;
			},
		},
		{ decodeEntities: true, recognizeSelfClosing: true },
	);
	parser.write(source);
	parser.end();
	for (let i = 0; i < protectedParts.length; i++)
		result = result.replace(`${prefix}${i}_END`, protectedParts[i].text);
	return result;
}

function legacyFriends(source: string): string | null {
	const read = (name: string) => {
		const literal = source.match(
			new RegExp(`export\\s+const\\s+${name}\\s*=\\s*([\\s\\S]*?);`),
		)?.[0];
		if (!literal) return undefined;
		const file = ts.createSourceFile(
			"content.ts",
			literal,
			ts.ScriptTarget.Latest,
			true,
		);
		const statement = file.statements.find(ts.isVariableStatement);
		return statement?.declarationList.declarations[0]?.initializer;
	};
	const site = read("site"),
		notes = read("notes");
	if (
		!site ||
		!notes ||
		!ts.isObjectLiteralExpression(site) ||
		!ts.isArrayLiteralExpression(notes)
	)
		return null;
	const fields = (node: ts.ObjectLiteralExpression) =>
		Object.fromEntries(
			node.properties
				.filter(ts.isPropertyAssignment)
				.flatMap((p) =>
					p.name && ts.isStringLiteral(p.initializer)
						? [
								[
									p.name.getText().replace(/^['"]|['"]$/g, ""),
									p.initializer.text,
								],
							]
						: [],
				),
		);
	const info = fields(site);
	const lines = ["# 友情链接", "", "## 本站信息", ""];
	for (const [key, label] of [
		["name", "站点名称"],
		["desc", "站点介绍"],
		["url", "站点地址"],
		["avatar", "头像地址"],
		["screenshot", "主页截图"],
		["email", "联系邮箱"],
	])
		if (info[key]) lines.push(`- ${label}：${info[key]}`);
	lines.push("", "## 申请说明", "");
	for (const note of notes.elements.filter(ts.isObjectLiteralExpression)) {
		const row = fields(note);
		lines.push(`- **${row.title || "说明"}**：${row.content || ""}`);
	}
	lines.push(
		"",
		"请先添加本站友链，再提供网站名称、介绍、网站地址、头像地址和 RSS 地址，便于核对与添加。",
		"",
	);
	return lines.join("\n");
}
