import type { SiteTheme } from "@/types/siteThemeConfig";

export function chooseSiteTheme(theme: SiteTheme): void {
	if (document.documentElement.dataset.siteTheme === theme) return;
	const destination = new URL(window.location.href);
	try {
		localStorage.setItem("rainzt.site-theme.v1", theme);
		destination.searchParams.delete("siteTheme");
	} catch {
		destination.searchParams.set("siteTheme", theme);
	}
	window.location.assign(destination.href);
}
