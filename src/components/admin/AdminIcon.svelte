<script lang="ts">
export let name = "articles";
const paths: Record<string, string> = {
	settings: "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8ZM12 3v3M12 18v3M3 12h3M18 12h3M5 5l2 2M17 17l2 2M5 19l2-2M17 7l2-2",
 friends:"M16 20v-2a4 4 0 0 0-4-4H7a4 4 0 0 0-4 4v2M12 3a4 4 0 1 1 0 8M14 12h7M18 9v6",
 pages:"M5 3h14v18H5V3ZM8 7h8M8 11h8M8 15h5",
 comments:"M3 4h18v13H8l-5 4V4ZM7 8h10M7 12h7",
 talks: "M4 4h16v12H9l-5 4V4Zm4 4h8M8 12h5",
	gallery: "M8 4h12v12H8V4ZM4 8v12h12M8 13l4-4 8 6",
	music: "M9 18V5l11-2v13M9 8l11-2M9 18a3 3 0 1 1-3-3c1.66 0 3 1.34 3 3Zm11-2a3 3 0 1 1-3-3c1.66 0 3 1.34 3 3Z",
	dashboard: "M4 4h6v6H4V4Zm10 0h6v6h-6V4ZM4 14h6v6H4v-6Zm10 0h6v6h-6v-6Z",
	visits: "M4 20h16M6 16v-4M12 16V5M18 16V9",
	users:
		"M16 20v-2a4 4 0 0 0-4-4H7a4 4 0 0 0-4 4v2M14 4a4 4 0 0 1 0 8M21 20v-2a4 4 0 0 0-3-3.87M14 7a4 4 0 1 1-8 0 4 4 0 0 1 8 0Z",
	articles: "M5 4h10l4 4v12H5V4Zm9 0v5h5M8 13h8M8 16h6",
	backgrounds: "M4 5h16v14H4V5Zm0 10 5-5 4 4 3-3 4 4M15 8h.01",
	documents: "M10 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14Zm5-2 5 5M7 8h6M7 11h4",
	images: "M8 4h12v12H8V4ZM4 8v12h12M8 13l4-4 8 6",
};
</script>
<svg aria-hidden="true" class="h-5 w-5 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d={paths[name]||paths.articles}/></svg>
