import { calendarHolidays } from "@/config/calendarHolidays";
import { getCalendarDayInfo } from "@/utils/calendar-day";

function enhanceCalendar() {
	const grid = document.getElementById("calendar-grid");
	const detail = document.getElementById("calendar-date-detail");
	const source = document.querySelector<HTMLAnchorElement>(
		"#calendar-holiday-source",
	);
	if (!grid || !detail || !source) return;
	let displayYear = new Date().getFullYear();
	for (const cell of grid.querySelectorAll<HTMLButtonElement>(
		".calendar-day[data-date]",
	)) {
		const key = cell.dataset.date;
		if (!key) continue;
		const [year, month, day] = key.split("-").map(Number);
		displayYear = year;
		if (cell.querySelector(".calendar-lunar")) continue;
		const date = new Date(year, month - 1, day, 12);
		const info = getCalendarDayInfo(date);
		const secondary = document.createElement("span");
		secondary.className = "calendar-lunar";
		secondary.textContent = info.label;
		cell.append(secondary);
		cell.classList.toggle("calendar-festival", info.observances.length > 0);
		if (info.status) {
			const badge = document.createElement("span");
			badge.className = `calendar-shift calendar-shift-${info.status}`;
			badge.textContent = info.status === "rest" ? "休" : "班";
			cell.append(badge);
		}
		const status =
			info.status === "rest"
				? `${info.holiday}假期`
				: info.status === "work"
					? "调休补班"
					: "";
		const text = [
			`${year}年${month}月${day}日`,
			`农历${info.lunar}`,
			...info.observances,
			status,
		]
			.filter(Boolean)
			.join(" · ");
		cell.title = text;
		cell.setAttribute(
			"aria-label",
			`${text}${cell.dataset.hasPost === "true" ? " · 有文章" : ""}`,
		);
		cell.addEventListener("click", () => {
			detail.textContent = text;
		});
	}
	const schedule = calendarHolidays[displayYear];
	source.hidden = !schedule;
	if (schedule) {
		source.href = schedule.source;
		source.textContent = `${displayYear}年调休公告 ↗`;
	}
	const notice = document.getElementById("calendar-holiday-notice");
	if (notice)
		notice.textContent = schedule ? "" : `${displayYear}年调休安排暂未收录`;
	detail.textContent = "点击日期，查看农历与节日";
}

document.addEventListener("calendar:render", enhanceCalendar);
document.addEventListener("astro:page-load", enhanceCalendar);
enhanceCalendar();
