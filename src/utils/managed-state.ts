import type { ManagedSite } from "@/types/managed-site";
declare global {
	interface Window {
		__yukinaSiteDefaults?: ManagedSite | null;
	}
}
export function getManagedAppearance(): ManagedSite["appearance"] | undefined {
	return typeof window !== "undefined"
		? window.__yukinaSiteDefaults?.appearance
		: undefined;
}
