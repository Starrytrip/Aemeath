import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { collectDocuments } from "./sync-documents";

test("collects article dates, links and privacy directly from source without a build", async () => {
	const root = await mkdtemp(path.join(tmpdir(), "yukina-source-sync-"));
	try {
		await mkdir(path.join(root, "travel"));
		await writeFile(
			path.join(root, "travel/index.md"),
			"---\ntitle: 旅行记录\npublished: 2026-11-09\naiSummary: 旅行摘要\n---\n\n正文内容",
		);
		await writeFile(
			path.join(root, "draft.md"),
			"---\ntitle: 草稿\npublished: 2026-11-08\ndraft: true\n---\n\n草稿正文",
		);
		await writeFile(
			path.join(root, "locked.mdx"),
			"---\ntitle: 加密文章\npublished: 2026-11-07\npassword: example\n---\n\n加密正文",
		);
		const docs = await collectDocuments(root);
		assert.equal(docs.length, 3);
		const article = docs.find(
			(doc) => doc.source_key === "astro-posts:travel/index.md",
		);
		assert.equal(article?.date, "2026-11-09");
		assert.equal(article?.url, "/posts/travel/");
		assert.equal(article?.ai_summary, "旅行摘要");
		assert.equal(article?.visibility, "public");
		assert.equal(
			docs.find((doc) => doc.title === "草稿")?.visibility,
			"private",
		);
		assert.equal(
			docs.find((doc) => doc.title === "加密文章")?.visibility,
			"private",
		);
	} finally {
		assert.equal(path.dirname(path.resolve(root)), path.resolve(tmpdir()));
		assert.match(path.basename(root), /^yukina-source-sync-/);
		await rm(root, { recursive: true, force: true });
	}
});

test("rejects invalid dates and duplicate route slugs before syncing", async () => {
	const root = await mkdtemp(path.join(tmpdir(), "yukina-invalid-sync-"));
	try {
		const file = path.join(root, "entry.md");
		await writeFile(
			file,
			"---\ntitle: 无效日期\npublished: 2026-02-30\n---\n\n正文",
		);
		await assert.rejects(collectDocuments(root), /有效的发布日期/);
		await writeFile(
			file,
			"---\ntitle: 文章\npublished: 2026-11-09\n---\n\n正文",
		);
		await mkdir(path.join(root, "entry"));
		await writeFile(
			path.join(root, "entry/index.md"),
			"---\ntitle: 同名路径\npublished: 2026-11-09\n---\n\n正文",
		);
		await assert.rejects(collectDocuments(root), /slug 重复/);
	} finally {
		assert.equal(path.dirname(path.resolve(root)), path.resolve(tmpdir()));
		assert.match(path.basename(root), /^yukina-invalid-sync-/);
		await rm(root, { recursive: true, force: true });
	}
});
