import { apiFetch } from "./api-client";
export interface PublishedArticle {
	title: string;
	slug: string;
	url: string;
	description: string;
	date: string;
	cover_url: string;
	category: string;
	tags: string[];
	words: number;
	is_pinned: boolean;
}
let pending: Promise<PublishedArticle[]> | null = null;
export function loadPublishedArticles(): Promise<PublishedArticle[]> {
	if (!pending) {
		pending = apiFetch<PublishedArticle[]>("/articles").finally(() => {
			pending = null;
		});
	}
	return pending;
}
export function articleLink(p: PublishedArticle): string {
	return `/reader/?slug=${encodeURIComponent(p.slug)}`;
}
export function articleTaxonomy(
	posts: PublishedArticle[],
	kind: "category" | "tag",
): { name: string; count: number; url: string }[] {
	const counts = new Map<string, number>();
	for (const p of posts)
		for (const name of kind === "tag" ? p.tags : [p.category || "未分类"])
			counts.set(name, (counts.get(name) || 0) + 1);
	return [...counts]
		.map(([name, count]) => ({
			name,
			count,
			url:
				kind === "category" && name === "未分类"
					? "/archive/?uncategorized=true"
					: `/archive/?${kind}=${encodeURIComponent(name)}`,
		}))
		.sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
}
