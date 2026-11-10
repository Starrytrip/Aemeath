import { SakuraManager } from "@/utils/sakura-animation";

const { sakuraConfig, baseUrl = "/" } = JSON.parse(
	document.getElementById("yukina-sakura-config")?.textContent || "{}",
);
// 创建全局樱花管理器实例
let globalSakuraManager = window.sakuraManager || null;
window.sakuraManager = globalSakuraManager;
let pendingSakuraStart = false;
let scrollResumeTimer = 0;

// 初始化樱花特效
function initSakura(config) {
	if (globalSakuraManager) {
		window.sakuraManager = globalSakuraManager;
		globalSakuraManager.updateConfig(config);
	} else {
		globalSakuraManager = new SakuraManager(
			config,
			`${baseUrl.replace(/\/$/, "")}/assets/images/effects/sakura.webp`,
		);
		window.sakuraManager = globalSakuraManager;
		if (config.enable) {
			globalSakuraManager.init();
		}
	}
}

function isSakuraPreferenceEnabled() {
	// 站长关闭可切换时，以配置为准，忽略访客本地偏好
	if (!sakuraConfig.switchable) return sakuraConfig.enable;

	try {
		const storedEnabled = localStorage.getItem("sakuraEnabled");
		return storedEnabled !== null
			? storedEnabled === "true"
			: sakuraConfig.enable;
	} catch {
		return sakuraConfig.enable;
	}
}

// Pure 主题下禁用所有装饰性特效，樱花一律不渲染
function isPureTheme() {
	return document.documentElement.dataset.siteTheme === "pure";
}

function syncSakuraForCurrentPage() {
	const shouldEnable =
		!isPureTheme() &&
		isSakuraPreferenceEnabled() &&
		document.visibilityState === "visible";
	const manager = window.sakuraManager;

	if (!window.sakuraInitialized || !manager) {
		initSakura(Object.assign({}, sakuraConfig, { enable: shouldEnable }));
		window.sakuraInitialized = true;
		return;
	}

	manager.config.enable = shouldEnable;
	if (shouldEnable) {
		if (!manager.getIsRunning()) {
			manager.init();
		} else if (!window.__friendsPortalActive) {
			manager.resume?.();
		}
	} else {
		manager.stop();
	}
}

function queueSakuraSync() {
	// Active petals survive route swaps; never defer resuming them to idle time.
	if (window.sakuraManager?.getIsRunning() || !isSakuraPreferenceEnabled()) {
		pendingSakuraStart = false;
		syncSakuraForCurrentPage();
		return;
	}
	if (pendingSakuraStart) return;
	pendingSakuraStart = true;

	const startWhenIdle = () => {
		if (!pendingSakuraStart) return;
		pendingSakuraStart = false;
		const start = () => syncSakuraForCurrentPage();
		if (typeof window.requestIdleCallback === "function") {
			window.requestIdleCallback(start, { timeout: 1200 });
		} else {
			window.setTimeout(start, 120);
		}
	};
	startWhenIdle();
}

// 樱花特效初始化
(() => {
	// 全局标记，确保樱花特效只初始化一次
	if (window.sakuraInitialized) {
		return;
	}

	// 初始化樱花特效的函数
	const setupSakura = () => {
		if (window.sakuraInitialized) return;
		// Defer only the initial creation until the browser is idle.
		queueSakuraSync();
	};

	// 页面加载完成后初始化樱花特效
	if (document.readyState === "loading") {
		document.addEventListener("DOMContentLoaded", setupSakura);
	} else {
		setupSakura();
	}
})();

// Swup/Astro 页面切换时同步樱花运行状态。
if (!window.sakuraRouteSyncBound) {
	window.sakuraRouteSyncBound = true;
	[
		"astro:page-load",
		"astro:after-swap",
		"swup:contentReplaced",
		"swup:content:replace",
		"swup:page:view",
	].forEach((eventName) => {
		document.addEventListener(eventName, queueSakuraSync);
	});
	window.addEventListener("pageshow", queueSakuraSync);

	const bindSakuraSwupHooks = () => {
		if (!window.swup?.hooks || window.sakuraSwupHooksBound) return;
		window.sakuraSwupHooksBound = true;
		window.swup.hooks.on("content:replace", queueSakuraSync);
		window.swup.hooks.on("page:view", queueSakuraSync);
	};
	document.addEventListener("swup:enable", bindSakuraSwupHooks);
	bindSakuraSwupHooks();
}

// 监听设置面板的樱花特效切换事件
window.addEventListener("sakuraToggle", () => {
	// 设置只保存偏好；实际运行状态还要受页面可见状态限制。
	queueSakuraSync();
});

// Pause only the SVG wave motion while scrolling; Sakura should keep drifting.
window.addEventListener(
	"scroll",
	() => {
		document.documentElement.classList.add("is-yukina-scrolling");
		window.clearTimeout(scrollResumeTimer);
		scrollResumeTimer = window.setTimeout(() => {
			document.documentElement.classList.remove("is-yukina-scrolling");
		}, 180);
	},
	{ passive: true },
);

document.addEventListener("visibilitychange", () => {
	if (document.visibilityState === "hidden") {
		window.clearTimeout(scrollResumeTimer);
		document.documentElement.classList.remove("is-yukina-scrolling");
		window.sakuraManager?.pause?.();
	} else {
		queueSakuraSync();
	}
});

// 全屏友链漫游打开时暂停装饰层，关闭后从原进度恢复。
window.addEventListener("friends-portal-open", () => {
	window.__friendsPortalActive = true;
	window.sakuraManager?.pause?.();
});
window.addEventListener("friends-portal-close", () => {
	window.__friendsPortalActive = false;
	if (document.visibilityState === "visible" && isSakuraPreferenceEnabled()) {
		window.sakuraManager?.resume?.();
	}
});

// Only rebuild petals when the site's effect parameters actually change.
let managedEffectKey = "";
const baseSpeed = structuredClone(sakuraConfig.speed);
function applyManagedSakura(config) {
 if (!config) return;
 const key = JSON.stringify([config.sakura_count,config.sakura_speed,config.sakura_enabled]);
 if (key === managedEffectKey) return;
 managedEffectKey = key;
 sakuraConfig.sakuraNum = config.sakura_count;
 sakuraConfig.enable = config.sakura_enabled;
 for (const axis of ["horizontal","vertical"]) {
  sakuraConfig.speed[axis].min = baseSpeed[axis].min * config.sakura_speed;
  sakuraConfig.speed[axis].max = baseSpeed[axis].max * config.sakura_speed;
 }
 if (globalSakuraManager) globalSakuraManager.updateConfig({...sakuraConfig,enable:isSakuraPreferenceEnabled() && !isPureTheme()});
 queueSakuraSync();
}
applyManagedSakura(window.__yukinaSiteDefaults?.appearance);
document.addEventListener("yukina:site-settings", e=>applyManagedSakura(e.detail.appearance));
