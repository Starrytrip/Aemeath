import {apiFetch} from "@/utils/api-client";
import { loadSiteSettings } from "@/utils/managed-site";
import type { ManagedSite, ManagedLink } from "@/types/managed-site";
import {
	applyThemeToDocument,
	applyWallpaperModeToDocument,
	getStoredTheme,
	getStoredWallpaperMode,
	getStoredNavbarBehavior,
	getStoredCardDecorationMode,
} from "@/utils/setting-utils";

function text(selector: string, value: string) {
	for (const el of document.querySelectorAll(selector)) el.textContent = value;
}
function links(target: Element | null, rows: ManagedLink[]) {
	if (!target) return;
	target.replaceChildren();
	for (const row of rows.filter((r) => r.enabled !== false)) {
		if (row.url.startsWith("copy:")) {
			const button = document.createElement("button");
			button.type = "button";
			button.textContent = row.name;
			button.className = "btn-plain rounded-lg px-2 py-1 text-sm";
			button.onclick = () => {
				void navigator.clipboard
					.writeText(row.url.slice(5))
					.then(() => {
						button.textContent = "已复制";
					})
					.catch(() => {
						button.textContent = "复制失败";
					});
			};
			target.append(button);
			continue;
		}
		const a = document.createElement("a");
		a.textContent = row.name;
		a.href = row.url;
		if (row.external) {
			a.target = "_blank";
			a.rel = "noopener noreferrer";
		}
		a.className = "btn-plain rounded-lg px-2 py-1 text-sm";
		target.append(a);
	}
}
function stored(key: string) {
	try {
		return localStorage.getItem(key);
	} catch {
		return null;
	}
}
function meta(name: string, value: string) {
	let el = document.querySelector<HTMLMetaElement>(`meta[name="${name}"]`);
	if (!el) {
		el = document.createElement("meta");
		el.name = name;
		document.head.append(el);
	}
	el.content = value;
}
function apply(config: ManagedSite) {
	window.__yukinaSiteDefaults = config;
	const root = document.documentElement;
 if(document.querySelector("[data-managed-friend-count]"))void apiFetch<{items:unknown[]}>("/friends").then(r=>text("[data-managed-friend-count]",String(r.items.length))).catch(()=>{});
	text(".home-wallpaper-card__content h1", config.basic.title);
	text(".home-wallpaper-card__subtitle--default", config.basic.subtitle);
	text(".home-wallpaper-card__identity", config.profile.name);
	text(".profile-name,.profile-handle", config.profile.name);
	text(".profile-bio,.profile-lead,.mobile-profile-bio", config.profile.bio);
	text(".mobile-profile-name,.pure-brand", config.basic.title);
	for (const img of document.querySelectorAll<HTMLImageElement>(
		".profile-avatar img,img.profile-avatar,.mobile-profile-avatar img,img.mobile-profile-avatar,.home-wallpaper-card__avatar-wrap img",
	)) {
		img.src = config.profile.avatar;
		img.removeAttribute("srcset");
		img
			.closest("picture")
			?.querySelectorAll("source")
			.forEach((s) => s.remove());
	}
	for (const brand of document.querySelectorAll(".nav-brand .brand-name")) {
		let label = brand.querySelector("[data-managed-brand]");
		if (!label) {
			for (const node of [...brand.childNodes])
				if (node.nodeType === Node.TEXT_NODE) node.remove();
			label = document.createElement("span");
			label.setAttribute("data-managed-brand", "");
			brand.append(label);
		}
		label.textContent = config.basic.title;
		const img = brand.querySelector("img");
		if (img) {
			img.src = config.basic.logo;
			img.removeAttribute("srcset");
			img
				.closest("picture")
				?.querySelectorAll("source")
				.forEach((s) => s.remove());
		} else if (config.basic.logo) {
			const image = document.createElement("img");
			image.src = config.basic.logo;
			image.alt = "";
			image.className = "mr-2 h-7 w-7 rounded-md object-contain";
			brand.prepend(image);
		}
	}
	links(document.querySelector(".profile-socials"), [
		...config.profile.links,
		{ name: "RSS", url: "/rss.xml" },
	]);
	text(
		".site-footer__meta>span:first-child,.pure-footer__copyright>span:first-child",
		config.footer.copyright,
	);
	links(document.querySelector(".site-footer__records"), config.footer.records);
	links(document.querySelector(".site-footer__nav"), [
		{ name: "博客", url: "/" },
		...config.profile.links,
	]);
	const announcement = document.querySelector(
		"widget-layout.announcement-card",
	);
	if (announcement) {
		const content = announcement.querySelector("#announcement>div");
		text(
			"widget-layout.announcement-card .widget-name",
			config.announcement.title,
		);
		if (content) {
			content.replaceChildren();
			const p = document.createElement("p");
			p.className = "mb-3 whitespace-pre-line text-sm leading-7";
			p.textContent = config.announcement.content;
			content.append(p);
			const row = document.createElement("div");
			row.className = "flex flex-wrap items-center gap-2";
			links(row, config.announcement.links);
			if (config.announcement.closable) {
				const close = document.createElement("button");
				close.type = "button";
				close.textContent = "×";
				close.ariaLabel = "关闭公告";
				close.className = "ml-auto rounded-lg px-2 py-1";
				close.onclick = () => {
					(announcement as HTMLElement).style.display = "none";
				};
				row.append(close);
			}
			content.append(row);
		}
		(announcement as HTMLElement).style.display = config.announcement.enabled
			? ""
			: "none";
	}
	const post =
		location.pathname.startsWith("/posts/") ||
		location.pathname.startsWith("/reader/");
	for (const w of config.sidebar.widgets) {
		const el = document.querySelector<HTMLElement>(
			`[data-managed-widget="${w.type}"]`,
		);
		const target = document.querySelector(
			`[data-managed-column="${w.side}"][data-managed-position="${w.sticky ? "sticky" : "top"}"]`,
		);
		if (el && target) {
			target.append(el);
			el.style.display =
				w.enabled && (!post || w.show_on_post) && (post || !w.only_on_post)
					? ""
					: "none";
		}
	}
	meta("description", config.basic.description);
	meta("keywords", config.basic.keywords);
	meta("author", config.profile.name);
	if (config.basic.favicon) {
		for (const icon of document.querySelectorAll<HTMLLinkElement>(
			'link[rel="icon"]',
		)) {
			icon.href = config.basic.favicon;
			icon.removeAttribute("sizes");
		}
	}
	if (location.pathname === "/")
		document.title = `${config.basic.title}${config.basic.subtitle ? ` · ${config.basic.subtitle}` : ""}`;
	root.dataset.navbarBehavior = getStoredNavbarBehavior();
	root.dataset.cardDecoration = getStoredCardDecorationMode();
	if (!stored("hue"))
		root.style.setProperty("--hue", String(config.appearance.hue));
	applyThemeToDocument(getStoredTheme());
	if (root.dataset.siteTheme !== "pure")
		applyWallpaperModeToDocument(getStoredWallpaperMode(), false);
	document.dispatchEvent(
		new CustomEvent("yukina:site-settings", { detail: config }),
	);
}
let busy = false;
async function refresh() {
	if (busy) return;
	busy = true;
	try {
		const config = await loadSiteSettings();
		if (config) apply(config);
	} catch {
		/* Keep the compiled defaults if the backend is temporarily unavailable. */
	} finally {
		busy = false;
	}
}
if (window.__yukinaSiteDefaults) {
	if (document.readyState === "loading")
		document.addEventListener(
			"DOMContentLoaded",
			() => apply(window.__yukinaSiteDefaults!),
			{ once: true },
		);
	else apply(window.__yukinaSiteDefaults);
}
for (const name of [
	"astro:page-load",
	"swup:page:view",
	"swup:contentReplaced",
	"yukina:settings-updated",
])
	document.addEventListener(name, () => void refresh());
document.addEventListener("swup:enable", () => {
	const swup = (
		window as Window & {
			swup?: { hooks: { on: (name: string, cb: () => void) => void } };
		}
	).swup;
	swup?.hooks.on("page:view", () => void refresh());
});
void refresh();
