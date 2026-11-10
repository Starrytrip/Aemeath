export interface GalleryPhoto {
	id: string;
	url: string;
	caption: string;
}
export interface ManagedAlbum {
	id: number;
	slug: string;
	name: string;
	description: string;
	date: string;
	location: string;
	cover: string;
	tags: string[];
	photos: GalleryPhoto[];
	published: boolean;
	sort_order: number;
}
