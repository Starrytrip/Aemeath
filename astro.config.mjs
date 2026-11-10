import { setMaxListeners } from "node:events";
import { request as httpRequest } from "node:http";
import { fileURLToPath } from "node:url";
import { unified } from "@astrojs/markdown-remark";
import sitemap from "@astrojs/sitemap";
import svelte from "@astrojs/svelte";
import { pluginCollapsibleSections } from "@expressive-code/plugin-collapsible-sections";
import { pluginLineNumbers } from "@expressive-code/plugin-line-numbers";
import swup from "@swup/astro";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "astro/config";
import expressiveCode from "astro-expressive-code";
import icon from "astro-icon";
import katex from "katex";
import rehypeAutolinkHeadings from "rehype-autolink-headings";
import rehypeComponents from "rehype-components"; /* Render the custom directive content */
import rehypeKatex from "rehype-katex";
import "katex/dist/contrib/mhchem.mjs"; // 加载 mhchem 扩展
import cloudflare from "@astrojs/cloudflare";
import mdx from "@astrojs/mdx";
import { pluginCollapsible } from "expressive-code-collapsible"; /* Collapsible */
import { pluginLanguageBadge } from "expressive-code-language-badge"; /* Language Badge */
import rehypeCallouts from "rehype-callouts";
import rehypeSlug from "rehype-slug";
import remarkAdmonitionToBlockquoteCallout from "remark-admonition-to-blockquote-callout";
import remarkDirective from "remark-directive"; /* Handle directives */
import remarkMath from "remark-math";
import remarkSectionize from "remark-sectionize";
import { expressiveCodeConfig, fontConfig, fontsList, plantumlConfig, siteConfig } from "./src/config";
import { collectUsedFontCssVars } from "./src/utils/fontHelper";
import I18nKey from "./src/i18n/i18nKey";
import { i18n } from "./src/i18n/translation";
import { fontProviders } from "astro/config";
import { GithubCardComponent } from "./src/plugins/rehype-component-github-card.mjs";
import rehypeEmailProtection from "./src/plugins/rehype-email-protection.mjs";
import rehypeExternalLinks from "./src/plugins/rehype-external-links.mjs";
import rehypeFigure from "./src/plugins/rehype-figure.mjs";
import { rehypeMermaid } from "./src/plugins/rehype-mermaid.mjs";
import { rehypePlantuml } from "./src/plugins/rehype-plantuml.mjs";
import { parseDirectiveNode } from "./src/plugins/remark-directive-rehype.js";
import { remarkExcerpt } from "./src/plugins/remark-excerpt.js";
import { remarkImageGrid } from "./src/plugins/remark-image-grid.js";
import { remarkMermaid } from "./src/plugins/remark-mermaid.js";
import { remarkPlantuml } from "./src/plugins/remark-plantuml.js";
import { remarkReadingTime } from "./src/plugins/remark-reading-time.mjs";
import { remarkWikiLink } from "./src/plugins/remark-wiki-link.mjs";
import { rehypeCodeGroup } from "./src/plugins/rehype-code-group.mjs";

if (process.env.NODE_ENV === "development") {
	setMaxListeners(20);
}

const adapter = process.env.CF_WORKERS
	? cloudflare({
			prerenderEnvironment: "node",
		})
	: undefined;

/**
 * 开发环境把 /api/v1 转发到本地 Rust 后端（backend/，默认 127.0.0.1:8787）。
 *
 * 必须放在 Astro 开发中间件之前，否则未匹配到页面的请求会先被 Astro 的 404 接管。
 * `/api` 下的静态 JSON 端点不在此前缀内，保持原样。
 */
function apiDevProxy() {
	const handle = (req, res, next) => {
		const publicFeeds = {"/rss.xml":"/api/v1/feed.xml","/sitemap.xml":"/api/v1/sitemap.xml","/sitemap-index.xml":"/api/v1/sitemap.xml"};
        const mapped = publicFeeds[req.url?.split("?")[0]];
        if (!mapped && !req.url?.startsWith("/api/v1")) return next();
		const proxyReq = httpRequest(
			{
				host: "127.0.0.1",
				port: 8787,
				path: mapped || req.url,
				method: req.method,
				headers: { ...req.headers, host: "127.0.0.1:8787" },
			},
			(proxyRes) => {
				res.writeHead(proxyRes.statusCode || 502, proxyRes.headers);
				proxyRes.pipe(res);
			},
		);
		proxyReq.on("error", () => {
			if (!res.headersSent) {
				res.writeHead(502, { "Content-Type": "text/plain; charset=utf-8" });
			}
			res.end("后端服务未启动（127.0.0.1:8787），请先在 backend/ 目录执行 cargo run");
		});
		req.pipe(proxyReq);
	};

	return {
		name: "yukina-api-dev-proxy",
		apply: "serve",
		configureServer(server) {
			// 在全部 configureServer 钩子执行完后插到最前面
			return () => {
				server.middlewares.stack.unshift({ route: "", handle });
			};
		},
	};
}

// https://astro.build/config
export default defineConfig({
	site: siteConfig.site_url,

	base: "/",
	trailingSlash: "always",

	// 字体配置 - 只加载实际使用的字体，跳过未引用的以加快构建
	fonts: (() => {
		// 禁用字体功能时直接返回空数组，跳过 Astro Font API 集成
		if (!fontConfig.enable) return [];

		const used = collectUsedFontCssVars(fontConfig);
		return fontsList
			.filter((f) => used.has(f.cssVariable))
			.map((f) => {
				let provider;
				switch (f.provider) {
					case "google": provider = fontProviders.google(); break;
					case "fontsource": provider = fontProviders.fontsource(); break;
					case "local": provider = fontProviders.local(); break;
					case "bunny": provider = fontProviders.bunny(); break;
					case "fontshare": provider = fontProviders.fontshare(); break;
					case "npm": provider = fontProviders.npm(); break;
					default: provider = f.provider;
				}
				return { ...f, provider };
			});
	})(),

	adapter,

	// 图像优化配置
	image: {
		// 全局响应式布局
		layout: "constrained",
	},

	integrations: [
		{
			name: "theme-hydration",
			hooks: {
				"astro:config:setup": ({ addClientDirective }) => {
					addClientDirective({
						name: "theme",
						entrypoint: fileURLToPath(new URL("./src/directives/theme.ts", import.meta.url)),
					});
				},
			},
		},
		swup({
			theme: false,
			animationClass: "transition-swup-", // see https://swup.js.org/options/#animationselector
			// the default value `transition-` cause transition delay
			// when the Tailwind class `transition-all` is used
			containers: [
				"#banner-overlay-container",
				"#banner-dim-container",
				"#swup-container",
				"#left-sidebar-dynamic",
				"#right-sidebar-dynamic",
				"#floating-toc-wrapper",
			],
			smoothScrolling: false,
			cache: true,
			preload: true,
			accessibility: true,
			// The portfolio is a deliberately full-bleed layout and does not use the
			// standard MainGridLayout Swup containers. Crossing this layout boundary
			// with a partial DOM swap leaves stale containers/styles behind, so both
			// directions use a normal document navigation instead.
			ignore: (targetUrl) => {
				// Pure owns navigation through Astro and persists its header.
				if (document.documentElement.dataset.siteTheme === "pure") return true;
				const targetPath = targetUrl.split(/[?#]/)[0] || "/";
				const currentPath = window.location.pathname;
				return targetPath === "/portfolio/" || currentPath === "/portfolio/";
			},
			updateHead: true,
			updateBodyClass: false,
			globalInstance: true,
			// 滚动相关配置优化
			resolveUrl: (url) => url,
			animateHistoryBrowsing: false,
			skipPopStateHandling: (event) => {
				if (document.documentElement.dataset.siteTheme === "pure") return true;
				// 跳过锚点链接的处理，让浏览器原生处理
				return event.state?.url?.includes("#");
			},
		}),
		icon({
			include: {
				"material-symbols": ["*"],
				"fa7-brands": ["*"],
				"fa7-regular": ["*"],
				"fa7-solid": ["*"],
				"simple-icons": ["*"],
				mdi: ["*"],
				mingcute: ["*"],
			},
		}),
		expressiveCode({
			themes: [expressiveCodeConfig.darkTheme, expressiveCodeConfig.lightTheme],
			useDarkModeMediaQuery: false,
			themeCssSelector: (theme) => `[data-theme='${theme.name}']`,
			plugins: [
				// pluginLanguageBadge 配置 - 从expressiveCodeConfig读取设置
				...(expressiveCodeConfig.pluginLanguageBadge?.enable === true
					? [pluginLanguageBadge()]
					: []),
				pluginCollapsibleSections(),
				pluginLineNumbers(),
				// pluginCollapsible 配置 - 从expressiveCodeConfig读取设置，使用i18n文本
				...(expressiveCodeConfig.pluginCollapsible?.enable === true
					? [
							pluginCollapsible({
								lineThreshold:
									expressiveCodeConfig.pluginCollapsible.lineThreshold || 15,
								previewLines:
									expressiveCodeConfig.pluginCollapsible.previewLines || 8,
								defaultCollapsed:
									expressiveCodeConfig.pluginCollapsible.defaultCollapsed ??
									true,
								expandButtonText: i18n(I18nKey.codeCollapsibleShowMore),
								collapseButtonText: i18n(I18nKey.codeCollapsibleShowLess),
								expandedAnnouncement: i18n(I18nKey.codeCollapsibleExpanded),
								collapsedAnnouncement: i18n(I18nKey.codeCollapsibleCollapsed),
							}),
						]
					: []),
			],
			defaultProps: {
				wrap: false,
				overridesByLang: {
					shellsession: {
						showLineNumbers: false,
					},
				},
			},
			styleOverrides: {
				borderRadius: "0.75rem",
				codeFontSize: "0.875rem",
				codeFontFamily:
					"var(--font-jetbrains-mono), ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace",
				codeLineHeight: "1.5rem",
				frames: {},
				textMarkers: {
					delHue: 0,
					insHue: 180,
					markHue: 250,
				},
				languageBadge: {
					fontSize: "0.75rem",
					fontWeight: "bold",
					borderRadius: "0.25rem",
					opacity: "1",
					borderWidth: "0px",
					borderColor: "transparent",
				},
			},
			frames: {
				showCopyToClipboardButton: true,
			},
		}),
		svelte(),

		mdx(),
	],
	markdown: {
		processor: unified({
			remarkPlugins: [
				...(siteConfig.post.rehypeCallouts.enablePythonMarkdownAdmonitions !== false
					? [remarkAdmonitionToBlockquoteCallout]
					: []),
				remarkMath,
				remarkReadingTime,
				remarkImageGrid,
				remarkExcerpt,
				remarkDirective,
				remarkWikiLink,
				remarkSectionize,
				parseDirectiveNode,
				remarkMermaid,
				[remarkPlantuml, plantumlConfig],
			],
			rehypePlugins: [
				[rehypeKatex, { katex }],
				[rehypeCallouts, { theme: siteConfig.post.rehypeCallouts.theme }],
				rehypeSlug,
				rehypeCodeGroup,
				rehypeMermaid,
				rehypePlantuml,
				rehypeFigure,
				[rehypeExternalLinks, { siteUrl: siteConfig.site_url }],
				[rehypeEmailProtection, { method: "base64" }], // 邮箱保护插件，支持 'base64' 或 'rot13'
				[
					rehypeComponents,
					{
						components: {
							github: GithubCardComponent,
						},
					},
				],
				[
					rehypeAutolinkHeadings,
					{
						behavior: "append",
						properties: {
							className: ["anchor"],
						},
						content: {
							type: "element",
							tagName: "span",
							properties: {
								className: ["anchor-icon"],
								"data-pagefind-ignore": true,
							},
							children: [
								{
									type: "text",
									value: "#",
								},
							],
						},
					},
				],
			],
		}),
	},
	vite: {
		plugins: [tailwindcss(), apiDevProxy()],
		server: {
			watch: {
				ignored: ["**/package/**"],
			},
		},
		resolve: {
			alias: {
				"@rehype-callouts-theme": `rehype-callouts/theme/${siteConfig.post.rehypeCallouts.theme}`,
			},
		},
		build: {
			minify: "esbuild",
			esbuildOptions: {
				minify: true,
				// 删除 debugger 语句；console.log / console.debug 无副作用，未使用返回值时会被 dead code elimination 移除，
				// console.warn / console.error 保留，确保生产环境出错时仍有日志可查
				drop: ["debugger"],
				pure: ["console.log", "console.debug"],
			},
			rollupOptions: {
				onwarn(warning, warn) {
					// temporarily suppress this warning
					if (
						warning.message.includes("is dynamically imported by") &&
						warning.message.includes("but also statically imported by")
					) {
						return;
					}
					warn(warning);
				},
			},
			// CSS 优化
			cssCodeSplit: true,
			cssMinify: "esbuild",
			assetsInlineLimit: 4096,
		},
	},
});
