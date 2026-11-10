import { DatabaseSync, backup } from "node:sqlite";
import { mkdir, readFile, writeFile, access } from "node:fs/promises";
import { spawn } from "node:child_process";
import { parseArgs } from "node:util";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { randomUUID } from "node:crypto";
import { parse } from "yaml";
import {
	imageReferences,
	localImage,
	replaceImages,
} from "./article-image-utils";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const { values } = parseArgs({
	options: {
		apply: { type: "boolean" },
		database: { type: "string" },
		"data-dir": { type: "string" },
		runner: { type: "string" },
	},
});
let database = values.database
	? path.resolve(values.database)
	: path.join(root, "backend/data/yukina.db");
if (!values.database) {
	try {
		await access(database);
	} catch {
		database = path.join(root, "backend/data/aemeath.db");
	}
}
const dataDir = values["data-dir"]
	? path.resolve(values["data-dir"])
	: path.dirname(database);
const db = new DatabaseSync(database, { readOnly: true });
type Row = {
	id: number;
	revision: number;
	source_key: string;
	markdown: string;
	cover_url: string;
};
const rows = db
	.prepare(
		"SELECT id,revision,source_key,markdown,cover_url FROM documents ORDER BY id",
	)
	.all() as unknown as Row[];
const images = new Map<string, { path: string; url: string }>();
const articles: (Row & { old_markdown: string; old_cover: string })[] = [];
const missing: string[] = [];
for (const row of rows) {
	const replacements = new Map<string, string>();
	const refs = await imageReferences(row.markdown);
	let cover = row.cover_url;
	if (!cover && row.source_key.startsWith("astro-posts:")) {
		try {
			const source = await readFile(
				path.join(
					root,
					"src/content/posts",
					row.source_key.slice("astro-posts:".length),
				),
				"utf8",
			);
			const match = source.match(
				/^\uFEFF?---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/,
			);
			const data = match ? parse(match[1]) : {};
			if (typeof data.image === "string" && data.image && data.image !== "api")
				cover = data.image;
		} catch {
			/* 后台新增文章不要求存在本地源文件。 */
		}
	}
	for (const reference of new Set([
		...refs.map((ref) => ref.url),
		...(cover ? [cover] : []),
	])) {
		try {
			const asset = await localImage(root, row.source_key, reference);
			if (asset) {
				images.set(asset.url, { path: asset.path, url: asset.url });
				replacements.set(reference, asset.url);
			}
		} catch (error) {
			missing.push(
				`${row.source_key} → ${reference}: ${error instanceof Error ? error.message : error}`,
			);
		}
	}
	const markdown = replaceImages(row.markdown, refs, replacements);
	cover = replacements.get(cover) || cover;
	if (markdown !== row.markdown || cover !== row.cover_url)
		articles.push({
			...row,
			old_markdown: row.markdown,
			old_cover: row.cover_url,
			markdown,
			cover_url: cover,
		});
}
console.log(
	`迁移预览：${articles.length} 篇文章，${images.size} 张去重原图，${missing.length} 个无法处理的引用。`,
);
if (missing.length) {
	console.error(missing.join("\n"));
	db.close();
	process.exitCode = 1;
} else if (!values.apply) {
	db.close();
	console.log("未写入。确认预览后使用 --apply；原始前端图片不会删除。");
} else {
	const directory = path.join(dataDir, "backups");
	await mkdir(directory, { recursive: true });
	const name = `article-images-${new Date().toISOString().replace(/[:.]/g, "-")}-${randomUUID()}`;
	const snapshot = path.join(directory, `${name}.db`);
	await backup(db, snapshot);
	db.close();
	const plan = path.join(directory, `${name}.json`);
	await writeFile(
		plan,
		JSON.stringify({ images: [...images.values()], articles }),
	);
	console.log(`数据库备份：${snapshot}`);
	const command = values.runner ? path.resolve(values.runner) : "cargo";
	const args = values.runner
		? ["migrate-article-images", plan]
		: [
				"run",
				"-p",
				"yukina-api",
				"--manifest-path",
				path.join(root, "backend/Cargo.toml"),
				"--",
				"migrate-article-images",
				plan,
			];
	const code = await new Promise<number>((resolve, reject) => {
		const child = spawn(command, args, {
			cwd: path.join(root, "backend"),
			windowsHide: true,
			stdio: "inherit",
			env: {
				...process.env,
				DATABASE_URL: database,
				DATA_DIR: dataDir,
				RUST_LOG: "warn",
			},
		});
		child.once("error", reject);
		child.once("exit", (code) => resolve(code ?? 1));
	});
	if (code) process.exitCode = code;
}
