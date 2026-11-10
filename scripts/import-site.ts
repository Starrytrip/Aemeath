import { readFile, mkdir, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { DatabaseSync, backup } from "node:sqlite";
import { cleanPageMarkdown } from "./page-content-utils";
import { siteDefaults, friendDefaults } from "../src/utils/site-defaults";
const apply = process.argv.includes("--apply");
function option(name: string) {
	const i = process.argv.indexOf(name);
	return i < 0 ? undefined : process.argv[i + 1];
}
const database = resolve(
	option("--database") ||
		(existsSync("backend/data/yukina.db")
			? "backend/data/yukina.db"
			: "backend/data/aemeath.db"),
);
const entries: Record<string, unknown> = {
	settings: siteDefaults,
	friends: friendDefaults,
};
for (const [slug, title] of [
	["about", "关于我"],
	["about-open-source", "开源项目介绍"],
	["guestbook", "留言页说明"],
	["friends", "友链页说明"],
]) {
	const path = `src/content/spec/${slug}.${slug === "friends" ? "mdx" : "md"}`;
	const markdown = (await readFile(path, "utf8")).replace(
		/^---\r?\n[\s\S]*?\r?\n---\r?\n/,
		"",
	);
	entries[`page-${slug}`] = {
		title,
		markdown: await cleanPageMarkdown(markdown),
		published: true,
	};
}
console.log(
	`准备从前端配置导入 ${Object.keys(entries).length} 个条目，已有后台配置会保留。`,
);
if (!apply) {
	console.log(
		"预览完成。添加 --apply 执行，先备份数据库。支持 --database 和 --runner。",
	);
	process.exit(0);
}
const backups = resolve(database, "../backups");
await mkdir(backups, { recursive: true });
const stamp = new Date().toISOString().replaceAll(":", "-");
if (existsSync(database)) {
	const db = new DatabaseSync(database, { readOnly: true });
	try {
		const file = resolve(backups, `site-settings-${stamp}.db`);
		await backup(db, file);
		console.log(`数据库备份：${file}`);
	} finally {
		db.close();
	}
}
const plan = resolve(backups, `site-settings-${stamp}.json`);
await writeFile(plan, JSON.stringify(entries, null, 2));
const runner = option("--runner");
const args = runner
	? ["import-site", plan]
	: [
			"run",
			"-p",
			"yukina-api",
			"--manifest-path",
			resolve("backend/Cargo.toml"),
			"--",
			"import-site",
			plan,
		];
const result = spawnSync(runner ? resolve(runner) : "cargo", args, {
	stdio: "inherit",
	cwd: resolve("backend"),
	env: { ...process.env, DATABASE_URL: database, RUST_LOG: "warn" },
});
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status || 1);
