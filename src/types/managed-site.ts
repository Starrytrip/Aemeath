export interface ManagedLink {
	name: string;
	url: string;
	icon?: string;
	external?: boolean;
	enabled?: boolean;
	children?: ManagedLink[];
}
export interface ManagedWidget {
	type: string;
	side: "left" | "right";
	enabled: boolean;
	show_on_post: boolean;
	only_on_post: boolean;
	sticky: boolean;
}
export interface ManagedSite {
	basic: {
		title: string;
		subtitle: string;
		description: string;
		keywords: string;
		site_url: string;
		logo: string;
		favicon: string;
		start_date: string;
	};
	profile: { name: string; bio: string; avatar: string; links: ManagedLink[] };
	announcement: {
		title: string;
		content: string;
		enabled: boolean;
		closable: boolean;
		links: ManagedLink[];
	};
	navigation: ManagedLink[];
	footer: { copyright: string; records: ManagedLink[] };
	appearance: {
		theme: "yukina" | "pure";
		mode: "light" | "dark" | "system";
		hue: number;
		list_mode: "grid" | "list";
		posts_per_page: number;
		wallpaper_mode: "banner" | "fullscreen" | "overlay" | "none";
		navbar_behavior: "auto" | "static";
		card_decoration: "soft" | "flat";
		sakura_enabled: boolean;
		sakura_count: number;
		sakura_speed: number;
		intro_enabled: boolean;
		carousel_interval: number;
	};
	sidebar: { widgets: ManagedWidget[] };
}
export interface ManagedFriend {
	id: string;
	title: string;
	description: string;
	siteurl: string;
	imgurl: string;
	rss: string;
	weight: number;
	enabled: boolean;
	recommended: boolean;
}
export interface ManagedPageData {
	title: string;
	markdown: string;
	published: boolean;
}
