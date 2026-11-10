export {};
interface Background {
	url: string;
	poster_url: string;
	media_type: string;
	theme: string;
	device: string;
}
const backgroundWindow = window as Window & {
	__managedBackgroundsRequest?: Promise<Background[]>;
};
let backgrounds: Background[] = [];
let backgroundsLoaded = false;
let generation = 0;
function revealWallpaper() {
	delete document.documentElement.dataset.managedBackgroundPending;
}
let current = "";
let position = 0;
const media = window.matchMedia("(max-width: 1023px)");
async function apply() {
	if (!backgroundsLoaded) return;
	const root = document.documentElement;
	const wrapper = document.getElementById("wallpaper-wrapper");
	if (!wrapper) return;
	const theme = root.classList.contains("dark") ? "dark" : "light";
	const items = backgrounds.filter(
		(b) =>
			(b.theme === "all" || b.theme === theme) &&
			b.device === (media.matches ? "mobile" : "desktop"),
	);
	const bg = items[position % Math.max(1, items.length)];
	const key = `${theme}-${media.matches}-${bg?.url}`;
	if (current === key && wrapper.querySelector("[data-managed-background]"))
		return;
	current = key;
	const token = ++generation;
	if (!bg) {
		wrapper.querySelector("[data-managed-background]")?.remove();
		revealWallpaper();
		return;
	}
	const overlay = document.createElement("div");
	overlay.dataset.managedBackground = "true";
	overlay.className =
		"absolute inset-0 z-[6] overflow-hidden pointer-events-none";
	let timeout: ReturnType<typeof setTimeout> | undefined;
	const ready = new Promise<void>((resolve, reject) => {
		timeout = setTimeout(
			() => reject(new Error("Background loading timed out")),
			8000,
		);
		if (bg.media_type === "video") {
			const video = document.createElement("video");
			video.onloadeddata = () => resolve();
			video.onerror = () =>
				reject(new Error("Background video failed to load"));
			video.src = bg.url;
			video.poster = bg.poster_url;
			video.autoplay = true;
			video.loop = true;
			video.muted = true;
			video.playsInline = true;
			video.className = "h-full w-full object-cover";
			overlay.append(video);
			void video.play().catch(() => {});
		} else {
			const img = document.createElement("img");
			img.onload = () => resolve();
			img.onerror = () => reject(new Error("Background image failed to load"));
			img.fetchPriority = "high";
			img.src = bg.url;
			img.alt = "";
			img.className = "h-full w-full object-cover";
			overlay.append(img);
		}
	});
	try {
		await ready;
		if (token !== generation) return;
		wrapper.querySelector("[data-managed-background]")?.remove();
		wrapper.append(overlay);
	} catch {
		if (token === generation) current = "";
	} finally {
		clearTimeout(timeout);
		if (token === generation) revealWallpaper();
	}
}
void (
	backgroundWindow.__managedBackgroundsRequest ??
	fetch("/api/v1/backgrounds", { signal: AbortSignal.timeout(5000) }).then(
		(r) => (r.ok ? r.json() : []),
	)
)
	.then((rows: Background[]) => {
		backgrounds = Array.isArray(rows) ? rows : [];
		backgroundsLoaded = true;
		apply();
	})
	.catch(revealWallpaper);
media.addEventListener("change", () => {
	position = 0;
	apply();
});
new MutationObserver(apply).observe(document.documentElement, {
	attributes: true,
	attributeFilter: ["class"],
});
for (const event of [
	"astro:page-load",
	"swup:page:view",
	"swup:contentReplaced",
])
	document.addEventListener(event, apply);
let timer: ReturnType<typeof setInterval>;
function scheduleCarousel(){clearInterval(timer);timer=setInterval(()=>{position++;void apply();},(window.__yukinaSiteDefaults?.appearance.carousel_interval || 15)*1000);}
document.addEventListener("yukina:site-settings",scheduleCarousel);
scheduleCarousel();
