import { createMarkdownProcessor } from "@astrojs/markdown-remark";
import type { Root, Nodes, Definition } from "mdast";
import { readFile, realpath } from "node:fs/promises";
import path from "node:path";
import { createHash } from "node:crypto";
import sharp from "sharp";

export type ImageReference = {
	url: string;
	start: number;
	end: number;
	alt: string;
	title?: string | null;
	definition?: string;
};
export type LocalImage = {
	path: string;
	url: string;
	bytes: Buffer;
	extension: string;
};

export async function imageReferences(
	markdown: string,
): Promise<ImageReference[]> {
	let result: ImageReference[] = [];
	const processor = await createMarkdownProcessor({
		syntaxHighlight: false,
		smartypants: false,
		remarkPlugins: [
			() => (tree: Root) => {
				const definitions = new Map<string, Definition>();
				const images: Nodes[] = [];
				function walk(node: Nodes) {
					if (node.type === "definition" && !definitions.has(node.identifier))
						definitions.set(node.identifier, node);
					if (node.type === "image" || node.type === "imageReference")
						images.push(node);
					if ("children" in node)
						for (const child of node.children) walk(child);
				}
				walk(tree);
				const seen = new Set<number>();
				for (const image of images) {
					const node =
						image.type === "imageReference"
							? definitions.get(image.identifier)
							: image;
					if (!node || !(node.type === "image" || node.type === "definition"))
						continue;
					const start = node.position?.start.offset;
					const end = node.position?.end.offset;
					if (start === undefined || end === undefined || seen.has(start))
						continue;
					seen.add(start);
					result.push({
						url: node.url,
						start,
						end,
						alt: node.type === "image" ? node.alt || "" : "",
						title: node.title,
						definition:
							node.type === "definition"
								? node.label || node.identifier
								: undefined,
					});
				}
				// 这里只需要语法树，不执行 HTML / MDX，也不让图片进入 Astro 构建流程。
				tree.children = [];
			},
		],
	});
	await processor.render(markdown);
	return result;
}

export function replaceImages(
	markdown: string,
	refs: ImageReference[],
	replacements: Map<string, string>,
): string {
	const escape = (text: string) =>
		text.replace(/\\/g, "\\\\").replace(/\[/g, "\\[").replace(/\]/g, "\\]");
	for (const ref of [...refs].sort((a, b) => b.start - a.start)) {
		const url = replacements.get(ref.url);
		if (!url) continue;
		const title = ref.title
			? ` "${ref.title.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`
			: "";
		const value = ref.definition
			? `[${escape(ref.definition)}]: <${url}>${title}`
			: `![${escape(ref.alt)}](<${url}>${title})`;
		markdown = markdown.slice(0, ref.start) + value + markdown.slice(ref.end);
	}
	return markdown;
}

export async function localImage(
	root: string,
	sourceKey: string,
	reference: string,
): Promise<LocalImage | null> {
	if (
		/^(?:[a-z][a-z\d+.-]*:|\/\/|#)/i.test(reference) ||
		reference.startsWith("/api/v1/")
	)
		return null;
	const pathname = decodeURIComponent(reference.split(/[?#]/)[0]);
	let base: string;
	let file: string;
	if (pathname.startsWith("/_astro/")) {
		base = path.join(root, "dist");
		file = path.join(base, pathname);
	} else if (pathname.startsWith("/")) {
		base = path.join(root, "public");
		file = path.join(base, pathname);
		try {
			await realpath(file);
		} catch (error) {
			if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
			// 兼容早期文章使用 /文章目录/图片名 指向源图的写法。
			base = path.join(root, "src/content/posts");
			file = path.join(base, pathname);
		}
	} else {
		if (!sourceKey.startsWith("astro-posts:"))
			throw new Error(`相对图片缺少原始文章目录：${reference}`);
		base = path.join(root, "src/content/posts");
		file = path.resolve(
			base,
			path.dirname(sourceKey.slice("astro-posts:".length)),
			pathname,
		);
	}
	const actual = await realpath(file);
	const allowed = await realpath(base);
	if (!actual.startsWith(allowed + path.sep))
		throw new Error(`图片路径越界：${reference}`);
	const bytes = await readFile(actual);
	if (bytes.length > 32 * 1024 * 1024)
		throw new Error(`原图超过 32 MiB：${reference}`);
	const metadata = await sharp(bytes, { animated: true }).metadata();
	const extension = metadata.format === "jpeg" ? "jpg" : metadata.format;
	if (!extension || !["png", "jpg", "webp", "gif"].includes(extension))
		throw new Error(`暂不支持的原图格式：${reference}`);
	const hash = createHash("sha256").update(bytes).digest("hex");
	return {
		path: actual,
		bytes,
		extension,
		url: `/api/v1/uploads/media-${hash}.${extension}`,
	};
}
