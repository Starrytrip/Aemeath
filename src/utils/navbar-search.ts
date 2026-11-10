import { apiFetch } from "@/utils/api-client";
import { url } from "@/utils/url-utils";

export type ArticleSuggestion = {
	id: number;
	slug: string;
	url: string;
	title: string;
	description: string;
	excerpt: string;
	date: string;
	cover_url: string;
	category: string;
	tags: string[];
	is_pinned: boolean;
};

export type ArticleSearchResponse = {
	items: ArticleSuggestion[];
	total: number;
	limit: number;
	offset: number;
};

export async function searchPublishedArticles(
	query: string,
	options: { limit?: number; offset?: number; signal?: AbortSignal } = {},
): Promise<ArticleSearchResponse> {
	const parameters = new URLSearchParams({
		q: query.trim(),
		limit: String(options.limit ?? 20),
		offset: String(options.offset ?? 0),
	});
	const timeout = AbortSignal.timeout(10_000);
	const data = await apiFetch<ArticleSearchResponse>(
		`/search/articles?${parameters}`,
		{
			cache: "no-store",
			signal: options.signal
				? AbortSignal.any([options.signal, timeout])
				: timeout,
		},
	);
	return {
		...data,
		items: data.items.map((item) => ({ ...item, url: url(item.url) })),
	};
}

export async function searchNavbarArticles(
	query: string,
	signal?: AbortSignal,
): Promise<ArticleSuggestion[]> {
	if (!query.trim()) return [];
	const data = await searchPublishedArticles(query, { limit: 6, signal });
	return data.items.map((item) => ({
		...item,
		description: item.description || item.excerpt,
	}));
}
