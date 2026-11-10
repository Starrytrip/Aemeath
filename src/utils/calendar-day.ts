import { Solar } from "lunar-typescript";
import { calendarHolidays } from "@/config/calendarHolidays";

export type CalendarDayInfo = {
	label: string;
	lunar: string;
	observances: string[];
	status?: "rest" | "work";
	holiday?: string;
};

export function getCalendarDayInfo(date: Date): CalendarDayInfo {
	const noon = new Date(
		date.getFullYear(),
		date.getMonth(),
		date.getDate(),
		12,
	);
	const solar = Solar.fromYmd(
		noon.getFullYear(),
		noon.getMonth() + 1,
		noon.getDate(),
	);
	const lunar = solar.getLunar();
	const day = lunar.getDay();
	const month = `${lunar.getMonthInChinese()}月`;
	const year = lunar.getYearInGanZhi();
	const dayName = lunar.getDayInChinese();
	const observances = [
		...new Set(
			[
				...solar.getFestivals(),
				...lunar.getFestivals(),
				lunar.getJieQi(),
			].filter(Boolean),
		),
	];
	const schedule = calendarHolidays[noon.getFullYear()];
	const key = `${noon.getFullYear()}-${String(noon.getMonth() + 1).padStart(2, "0")}-${String(noon.getDate()).padStart(2, "0")}`;
	const holiday = schedule?.rest.find(
		(range) => key >= range.from && key <= range.to,
	);
	return {
		label: observances[0] || (day === 1 ? month : dayName),
		lunar: `${year ? `${year}年` : ""}${month}${dayName}`,
		observances,
		status: schedule?.work.includes(key)
			? "work"
			: holiday
				? "rest"
				: undefined,
		holiday: holiday?.name,
	};
}
