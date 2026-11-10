// 将公开配置写入构建快照，让静态 HTML 的标题、SEO 和首屏与后台一致。
import { readFile, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { resolve } from "node:path";
const file = resolve("src/constants/managed-site.json");
let config: unknown = null;
const api = process.env.SITE_API_URL;
if (api) {
	const response = await fetch(`${api.replace(/\/$/, "")}/api/v1/site`, {
		signal: AbortSignal.timeout(10000),
	});
	if (!response.ok) throw new Error(`站点配置获取失败：${response.status}`);
	config = ((await response.json()) as { configuration: unknown })
		.configuration;
} else {
	const path =
		process.env.SITE_DATABASE ||
		(existsSync("backend/data/yukina.db")
			? "backend/data/yukina.db"
			: "backend/data/aemeath.db");
	if (existsSync(path)) {
		const db = new DatabaseSync(path, { readOnly: true });
		try {
			if (
				db
					.prepare("SELECT 1 FROM sqlite_master WHERE name='site_entries'")
					.get()
			) {
				const row = db
					.prepare("SELECT data FROM site_entries WHERE key='settings'")
					.get() as { data: string } | undefined;
				if (row) config = JSON.parse(row.data);
			}
		} finally {
			db.close();
		}
	}
}
if (config) {
	const text = `${JSON.stringify(config, null, 2)}\n`;
	const old = await readFile(file, "utf8").catch(() => "");
	if (old !== text) await writeFile(file, text);
	console.log("已同步站点公开配置到构建快照");
} else {
	console.log("未发现后台站点配置，保留现有构建默认值");
}
