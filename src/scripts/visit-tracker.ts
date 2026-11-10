import { getVisitorId } from "@/utils/api-client";
let lastNavigation = "";
let scheduled = false;
const visitor = getVisitorId();
const visitorId =
	visitor === "anonymous-visitor" ? crypto.randomUUID() : visitor;
function record(): void {
	scheduled = false;
	if (
		navigator.doNotTrack === "1" ||
		(navigator as Navigator & { globalPrivacyControl?: boolean })
			.globalPrivacyControl
	)
		return;
	if (/^\/(admin|api)(\/|$)/.test(location.pathname)) return;
	const navigation = location.pathname + location.search;
	if (navigation === lastNavigation) return;
	lastNavigation = navigation;
	const body = JSON.stringify({
		event_id: crypto.randomUUID(),
		visitor_id: visitorId,
		path: location.pathname,
		title: document.title.slice(0, 300),
		referrer: document.referrer,
	});
	void fetch("/api/v1/analytics/events", {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body,
		keepalive: true,
		credentials: "same-origin",
	}).catch(() => {});
}
function schedule(): void {
	if (scheduled) return;
	scheduled = true;
	setTimeout(record, 0);
}
if (document.readyState === "loading")
	document.addEventListener("DOMContentLoaded", schedule, { once: true });
else schedule();
for (const event of [
	"astro:page-load",
	"swup:page:view",
	"swup:contentReplaced",
	"swup:content:replace",
])
	document.addEventListener(event, schedule);
