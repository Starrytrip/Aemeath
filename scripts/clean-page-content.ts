import { DatabaseSync, backup } from "node:sqlite";
import { existsSync } from "node:fs";
import { mkdir } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { cleanPageMarkdown } from "./page-content-utils";
const args = process.argv.slice(2);
const index = args.indexOf("--database");
const path = resolve(
	index < 0
		? existsSync("backend/data/yukina.db")
			? "backend/data/yukina.db"
			: "backend/data/aemeath.db"
		: args[index + 1],
);
const apply = args.includes("--apply");
const db = new DatabaseSync(path, { readOnly: !apply });
try {
	const plan = [];
	for (const row of db
		.prepare(
			"SELECT key,data,revision FROM site_entries WHERE key LIKE 'page-%'",
		)
		.all() as { key: string; data: string; revision: number }[]) {
		const data = JSON.parse(row.data);
		const markdown = await cleanPageMarkdown(data.markdown);
		if (markdown !== data.markdown)
			plan.push({ ...row, next: JSON.stringify({ ...data, markdown }) });
	}
	console.log(`需要清理 ${plan.length} 个页面的样式标签。`);
	if (apply && plan.length) {
		const dir = resolve(dirname(path), "backups");
		await mkdir(dir, { recursive: true });
		const file = resolve(
			dir,
			`page-content-${new Date().toISOString().replaceAll(":", "-")}.db`,
		);
		await backup(db, file);
		console.log(`数据库备份：${file}`);
		db.exec("BEGIN IMMEDIATE");
		try {
			for (const row of plan) {
				const result = db
					.prepare(
						"UPDATE site_entries SET data=?,revision=revision+1,updated_at=? WHERE key=? AND revision=? AND data=?",
					)
					.run(
						row.next,
						new Date().toISOString().replace("T", " ").replace("Z", ""),
						row.key,
						row.revision,
						row.data,
					);
				if (result.changes !== 1)
					throw new Error(`${row.key} 已被修改，迁移回滚`);
			}
			db.exec("COMMIT");
		} catch (e) {
			db.exec("ROLLBACK");
			throw e;
		}
		console.log("页面正文清理完成，标题和发布状态保持不变。");
	} else if (!apply) console.log("添加 --apply 执行，执行前自动备份数据库。");
} finally {
	db.close();
}
