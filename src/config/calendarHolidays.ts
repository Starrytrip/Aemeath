export type HolidaySchedule = {
	source: string;
	rest: { name: string; from: string; to: string }[];
	work: string[];
};

// Only published mainland China schedules belong here; do not infer future shifts.
export const calendarHolidays: Record<number, HolidaySchedule> = {
	2026: {
		source:
			"https://www.beijing.gov.cn/zhengce/zhengcefagui/202511/t20251104_4258873.html",
		rest: [
			{ name: "元旦", from: "2026-01-01", to: "2026-01-03" },
			{ name: "春节", from: "2026-02-15", to: "2026-02-23" },
			{ name: "清明节", from: "2026-04-04", to: "2026-04-06" },
			{ name: "劳动节", from: "2026-05-01", to: "2026-05-05" },
			{ name: "端午节", from: "2026-06-19", to: "2026-06-21" },
			{ name: "中秋节", from: "2026-09-25", to: "2026-09-27" },
			{ name: "国庆节", from: "2026-10-01", to: "2026-10-07" },
		],
		work: [
			"2026-01-04",
			"2026-02-14",
			"2026-02-28",
			"2026-05-09",
			"2026-09-20",
			"2026-10-10",
		],
	},
	2027: {
		source: "https://www.forestry.gov.cn/lyj/1/szxx/20261105/695102.html",
		rest: [
			{ name: "元旦", from: "2027-01-01", to: "2027-01-03" },
			{ name: "春节", from: "2027-02-05", to: "2027-02-13" },
			{ name: "清明节", from: "2027-04-03", to: "2027-04-05" },
			{ name: "劳动节", from: "2027-05-01", to: "2027-05-05" },
			{ name: "端午节", from: "2027-06-09", to: "2027-06-09" },
			{ name: "中秋节", from: "2027-09-15", to: "2027-09-15" },
			{ name: "国庆节", from: "2027-10-01", to: "2027-10-07" },
		],
		work: [
			"2027-01-31",
			"2027-02-14",
			"2027-05-08",
			"2027-09-26",
			"2027-10-09",
		],
	},
};
