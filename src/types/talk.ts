export interface Talk {
	id: number;
	content: string;
	images: string[];
	published: boolean;
	is_pinned: boolean;
	revision: number;
	created_at: string;
	updated_at: string;
	published_at: string | null;
	likes: number;
	liked: boolean;
}
export interface TalkList {
	items: Talk[];
	total: number;
	page: number;
	page_size: number;
}
