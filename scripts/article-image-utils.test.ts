import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import sharp from "sharp";
import {
	imageReferences,
	replaceImages,
	localImage,
} from "./article-image-utils";

test("rewrites actual image nodes and references without changing code examples or surrounding text", async () => {
	const markdown =
		'# 中文 😀\n\n![透明图](./image.png "图标题")\n\n![引用图片][pic]\n\n[pic]: ./other.webp "引用标题"\n\n`![示例](./code.png)`\n\n```md\n![示例](./fence.png)\n```\n';
	const refs = await imageReferences(markdown);
	assert.deepEqual(
		refs.map((ref) => ref.url),
		["./image.png", "./other.webp"],
	);
	const result = replaceImages(
		markdown,
		refs,
		new Map([
			["./image.png", "/api/v1/uploads/media-a.png"],
			["./other.webp", "/api/v1/uploads/media-b.webp"],
		]),
	);
	assert.match(
		result,
		/!\[透明图\]\(<\/api\/v1\/uploads\/media-a.png> "图标题"\)/,
	);
	assert.match(
		result,
		/\[pic\]: <\/api\/v1\/uploads\/media-b.webp> "引用标题"/,
	);
	assert.ok(result.startsWith("# 中文 😀\n\n"));
	assert.ok(result.includes("`![示例](./code.png)`"));
	assert.ok(result.includes("```md\n![示例](./fence.png)\n```"));
	assert.deepEqual(
		(await imageReferences(result)).map((ref) => ref.url),
		["/api/v1/uploads/media-a.png", "/api/v1/uploads/media-b.webp"],
	);
});

test("resolves local images by article source, deduplicates and blocks path traversal", async () => {
	const root = await mkdtemp(path.join(tmpdir(), "yukina-image-utils-"));
	try {
		const dir = path.join(root, "src/content/posts/entry");
		await mkdir(dir, { recursive: true });
		await mkdir(path.join(root, "public"));
		const png = await sharp({
			create: {
				width: 2,
				height: 2,
				channels: 4,
				background: { r: 20, g: 60, b: 80, alpha: 0.5 },
			},
		})
			.png()
			.toBuffer();
		await writeFile(path.join(dir, "image.png"), png);
		await writeFile(path.join(root, "private.png"), png);
		const image = await localImage(
			root,
			"astro-posts:entry/index.md",
			"./image.png",
		);
		assert.ok(
			image?.url.match(/^\/api\/v1\/uploads\/media-[a-f0-9]{64}\.png$/),
		);
		assert.deepEqual(image.bytes, png);
		assert.equal(
			(await localImage(root, "astro-posts:entry/index.md", "/entry/image.png"))
				?.url,
			image.url,
		);
		assert.equal(
			await localImage(
				root,
				"astro-posts:entry/index.md",
				"https://example.com/image.png",
			),
			null,
		);
		assert.equal(
			await localImage(root, "astro-posts:entry/index.md", image.url),
			null,
		);
		await assert.rejects(
			localImage(root, "astro-posts:entry/index.md", "../../../../private.png"),
			/越界/,
		);
	} finally {
		assert.equal(path.dirname(path.resolve(root)), path.resolve(tmpdir()));
		assert.match(path.basename(root), /^yukina-image-utils-/);
		await rm(root, { recursive: true, force: true });
	}
});
