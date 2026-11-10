import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { slug } from "github-slugger";
import { parse } from "yaml";
import {
	imageReferences,
	localImage,
	replaceImages,
} from "./article-image-utils";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const contentRoot = path.join(root, "src/content/posts");
const apply = process.argv.includes("--apply");
const base = process.env.DOCUMENTS_API_URL || "http://127.0.0.1:8787";
const token = process.env.DOCUMENTS_ADMIN_TOKEN;

type Document = {
	source_key: string;
	title: string;
	url: string;
	markdown: string;
	category: string;
	tags: string[];
	visibility: "public" | "private";
	ai_summary?: string;
	date: string;
	cover_url?: string;
};

async function files(dir: string): Promise<string[]> {
	const entries = await readdir(dir, { withFileTypes: true });
	const result: string[] = [];
	for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
		if (entry.name.startsWith(".")) continue;
		const full = path.join(dir, entry.name);
		if (entry.isDirectory()) result.push(...(await files(full)));
		else if (/\.mdx?$/i.test(entry.name)) result.push(full);
	}
	return result;
}

export async function collectDocuments(
	directory = contentRoot,
): Promise<Document[]> {
	const result: Document[] = [];
	const ids = new Set<string>();
	for (const file of await files(directory)) {
		const relative = path.relative(directory, file).replaceAll("\\", "/");
		const source = await readFile(file, "utf8");
		const match = source.match(/^\uFEFF?---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/);
		if (!match) throw new Error(`${relative} 缺少 YAML frontmatter`);
		const data = parse(match[1]) as Record<string, unknown>;
		if (!data || typeof data.title !== "string")
			throw new Error(`${relative} 缺少标题`);
		const id =
			typeof data.slug === "string"
				? data.slug
				: relative
						.replace(/\.mdx?$/i, "")
						.split("/")
						.map((segment) => slug(segment))
						.join("/")
						.replace(/\/index$/, "");
		if (ids.has(id)) throw new Error(`文章 slug 重复：${id}`);
		ids.add(id);
		const privateDocument = data.draft === true || Boolean(data.password);
		const rawDate = data.published ?? data.date;
		const date =
			rawDate instanceof Date
				? rawDate.toISOString().slice(0, 10)
				: String(rawDate ?? "").slice(0, 10);
		const parsedDate = new Date(`${date}T12:00:00Z`);
		if (
			!/^\d{4}-\d{2}-\d{2}$/.test(date) ||
			!Number.isFinite(parsedDate.getTime()) ||
			parsedDate.toISOString().slice(0, 10) !== date
		)
			throw new Error(`${relative} 缺少有效的发布日期 published`);
		const markdown = source.slice(match[0].length).trim();
		if (!markdown) continue;
		result.push({
			source_key: `astro-posts:${relative}`,
			title: data.title,
			url: `/posts/${id}/`,
			date,
			cover_url:
				typeof data.image === "string" && data.image && data.image !== "api"
					? data.image
					: undefined,
			markdown,
			category: typeof data.category === "string" ? data.category : "",
			tags: Array.isArray(data.tags)
				? data.tags.filter((tag): tag is string => typeof tag === "string")
				: [],
			visibility: privateDocument ? "private" : "public",
			ai_summary:
				typeof data.aiSummary === "string" ? data.aiSummary : undefined,
		});
	}
	return result;
}

async function api<T = unknown>(route: string, body: unknown): Promise<T> {
	for (let attempt = 0; attempt < 5; attempt++) {
		const response = await fetch(new URL(route, base), {
			method: "POST",
			redirect: "error",
			signal: AbortSignal.timeout(60_000),
			headers: {
				...(body instanceof FormData
					? {}
					: { "Content-Type": "application/json" }),
				Authorization: `Bearer ${token}`,
			},
			body: body instanceof FormData ? body : JSON.stringify(body),
		});
		if (response.ok) return (await response.json()) as T;
		if (response.status === 429 && attempt < 4) {
			const retry = Number(response.headers.get("retry-after") || "2");
			await response.body?.cancel();
			await delay(
				Math.min(60, Math.max(1, Number.isFinite(retry) ? retry : 2)) * 1000,
			);
			continue;
		}
		throw new Error(`文档同步失败：HTTP ${response.status}`);
	}
}

async function main() {
	const documents = await collectDocuments();
	console.log(
		`文档 ${documents.length} 篇：公开 ${documents.filter((d) => d.visibility === "public").length}，私有 ${documents.filter((d) => d.visibility === "private").length}`,
	);
	if (!apply) {
		console.log(
			"预览完成，未写入后端。配置 DOCUMENTS_API_URL、DOCUMENTS_ADMIN_TOKEN 后加 --apply 执行同步。",
		);
		return;
	}
	const target = new URL(base);
	if (!token)
		throw new Error("请通过 DOCUMENTS_ADMIN_TOKEN 配置管理员访问令牌");
	if (
		target.protocol !== "https:" &&
		!(
			target.protocol === "http:" &&
			["localhost", "127.0.0.1", "[::1]"].includes(target.hostname)
		)
	)
		throw new Error("远程同步必须使用 HTTPS");
	const uploaded = new Map<string, string>();
	for (const document of documents) {
		const refs = await imageReferences(document.markdown);
		const replacements = new Map<string, string>();
		for (const reference of new Set([
			...refs.map((ref) => ref.url),
			...(document.cover_url ? [document.cover_url] : []),
		])) {
			const asset = await localImage(root, document.source_key, reference);
			if (!asset) continue;
			let url = uploaded.get(asset.url);
			if (!url) {
				const form = new FormData();
				form.append(
					"file",
					new Blob([new Uint8Array(asset.bytes)]),
					path.basename(asset.path),
				);
				const saved = await api<{ url: string }>(
					"/api/v1/admin/images/import",
					form,
				);
				url = saved.url;
				uploaded.set(asset.url, url);
			}
			replacements.set(reference, url);
		}
		document.markdown = replaceImages(document.markdown, refs, replacements);
		if (document.cover_url)
			document.cover_url =
				replacements.get(document.cover_url) || document.cover_url;
		await api("/api/v1/documents", document);
	}
	// 全部上传成功后才隐藏已从本地删除的文章，避免公开索引残留旧内容。
	await api("/api/v1/documents/reconcile", {
		source_keys: documents.map((d) => d.source_key),
	});
	console.log(
		"同步完成。未配置 embedding 时关键词搜索立即可用，语义索引等待配置。",
	);
}

if (
	process.argv[1] &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	main().catch((error) => {
		console.error(error instanceof Error ? error.message : "文档同步失败");
		process.exitCode = 1;
	});
}
