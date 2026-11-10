import { createServer, request } from "node:http";
import { parseArgs } from "node:util";
import { preview } from "astro";

// Astro 7 的静态预览在代理之前检查 trailingSlash，且不加载用户 Vite 插件。
// 在外层分流 API，让上传文件与配置都使用同一站点来源。
const { values } = parseArgs({
	options: {
		host: { type: "string", default: "127.0.0.1" },
		port: { type: "string", default: "4321" },
	},
});
const port = Number(values.port);
if (!Number.isInteger(port) || port < 1 || port > 65535) {
	throw new Error("预览端口必须是 1–65535 之间的整数");
}
const site = await preview({ server: { host: "127.0.0.1", port: 0 }, logLevel: "error" });
const server = createServer((req, res) => {
	const feedPath = {"/rss.xml":"/api/v1/feed.xml","/sitemap.xml":"/api/v1/sitemap.xml","/sitemap-index.xml":"/api/v1/sitemap.xml"}[req.url?.split("?")[0]];
	const api = Boolean(feedPath) || req.url === "/api/v1" || req.url?.startsWith("/api/v1/") || req.url?.startsWith("/api/v1?");
	const targetPort = api ? 8787 : site.port;
	const upstream = request({
		hostname: "127.0.0.1",
		port: targetPort,
		path: feedPath || req.url,
		method: req.method,
		headers: { ...req.headers, host: `127.0.0.1:${targetPort}` },
	}, (response) => {
		res.writeHead(response.statusCode || 502, response.headers);
		response.on("error", () => res.destroy());
		response.pipe(res);
	});
	upstream.on("error", () => {
		if (res.headersSent) { res.destroy(); return; }
		res.writeHead(502, { "Content-Type": "text/plain; charset=utf-8" });
		res.end(api ? "后端服务未启动，请在 backend/ 目录运行 cargo run（端口 8787）。" : "预览服务暂不可用。");
	});
	req.on("aborted", () => upstream.destroy());
	res.on("close", () => upstream.destroy());
	req.pipe(upstream);
});
server.on("error", async (error) => {
	console.error(error.message);
	await site.stop();
	process.exitCode = 1;
});
server.listen(port, values.host, () => {
	console.log(`Yukina preview: http://${values.host}:${port}/ (API → 127.0.0.1:8787)`);
});
async function stop() {
	server.closeAllConnections();
	server.close();
	await site.stop();
}
process.once("SIGINT", stop);
process.once("SIGTERM", stop);
