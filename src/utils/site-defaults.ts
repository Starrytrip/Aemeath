import { siteConfig } from "@/config/siteConfig";
import { profileConfig } from "@/config/profileConfig";
import { announcementConfig } from "@/config/announcementConfig";
import { navBarConfig } from "@/config/navBarConfig";
import { footerConfig } from "@/config/footerConfig";
import { siteThemeConfig } from "@/config/siteThemeConfig";
import { sakuraConfig } from "@/config/effectsConfig";
import { backgroundWallpaper } from "@/config/backgroundWallpaper";
import { sidebarLayoutConfig } from "@/config/sidebarConfig";
import { homePortfolioIntroSettings } from "@/config/homePortfolioIntro";
import { friendsConfig } from "@/config/friendsConfig";
import type { ManagedSite, ManagedFriend } from "@/types/managed-site";
export const siteDefaults: ManagedSite = {
	basic: {
		title: siteConfig.title,
		subtitle: siteConfig.subtitle,
		description: siteConfig.description || "",
		keywords: (siteConfig.keywords || []).join(", "),
		site_url: siteConfig.site_url,
		logo: siteConfig.navbar.logo?.value || "",
		favicon: siteConfig.favicon?.[0]?.src || "",
		start_date: siteConfig.siteStartDate || "2026-01-01",
	},
	profile: {
		name: profileConfig.name,
		bio: profileConfig.bio || "",
		avatar: profileConfig.avatar || "",
		links: profileConfig.links.map((l) => ({ ...l })),
	},
	announcement: {
		title: announcementConfig.title || "公告",
		content: announcementConfig.content,
		enabled: true,
		closable: announcementConfig.closable ?? true,
		links: (announcementConfig.links || []).map((l) => ({
			name: l.text,
			url: l.url,
			external: l.external,
			enabled: l.enable,
		})),
	},
	navigation: structuredClone(navBarConfig.links),
	footer: {
		copyright: `© ${new Date().getFullYear()} ${siteConfig.title}`,
		records: (footerConfig.records || []).map((r) => ({
			name: r.label,
			url: r.url,
		})),
	},
	appearance: {
		theme: siteThemeConfig.defaultTheme,
		mode: siteConfig.themeColor.defaultMode || "system",
		hue: siteConfig.themeColor.hue,
		list_mode: siteConfig.postListLayout.defaultMode,
		posts_per_page: siteConfig.pagination.postsPerPage,
		wallpaper_mode: backgroundWallpaper.mode,
		navbar_behavior: "static",
		card_decoration: "flat",
		sakura_enabled: sakuraConfig.enable,
		sakura_count: sakuraConfig.sakuraNum,
		sakura_speed: 1,
		intro_enabled: homePortfolioIntroSettings.defaultEnabled,
		carousel_interval:
			(backgroundWallpaper.common?.carousel?.interval || 6000) / 1000,
	},
	sidebar: {
		widgets: [
			...["left", "right"].flatMap((side) =>
				(side === "left"
					? sidebarLayoutConfig.leftComponents
					: sidebarLayoutConfig.rightComponents
				).map((w) => ({
					type: w.type,
					side: side as "left" | "right",
					enabled: w.enable,
					show_on_post: w.showOnPostPage !== false,
					only_on_post: w.hideOnNonPostPage === true,
					sticky: w.position === "sticky",
				})),
			),
		].filter((w) => w.type !== "advertisement"),
	},
};
export const friendDefaults: ManagedFriend[] = friendsConfig.map((f, i) => ({
	id: `friend-${i}`,
	title: f.title,
	description: f.desc,
	siteurl: f.siteurl,
	imgurl: f.imgurl,
	rss: Array.isArray(f.rss) ? f.rss[0] || "" : f.rss || "",
	weight: f.weight,
	enabled: f.enabled,
	recommended: f.recommended || false,
}));
