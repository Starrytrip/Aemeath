import type { ManagedSite } from "@/types/managed-site";
import { apiFetch } from "./api-client";
let cached: ManagedSite | null = null;
let fetchedAt = 0;
let pending: Promise<ManagedSite | null> | null = null;
export function loadSiteSettings(): Promise<ManagedSite | null> {
	if (Date.now() - fetchedAt < 500) return Promise.resolve(cached);
	if (!pending) {
		pending = apiFetch<{ configuration: ManagedSite | null }>("/site")
			.then((r) => {
				if (typeof window !== "undefined")
					window.__yukinaSiteDefaults = r.configuration;
				cached = r.configuration;
				fetchedAt = Date.now();
				return r.configuration;
			})
			.finally(() => {
				pending = null;
			});
	}
	return pending;
}
