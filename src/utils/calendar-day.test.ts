import assert from "node:assert/strict";
import test from "node:test";
import { getCalendarDayInfo } from "./calendar-day";

const day = (date: string) => {
	const [year, month, dateOfMonth] = date.split("-").map(Number);
	return getCalendarDayInfo(new Date(year, month - 1, dateOfMonth, 12));
};

test("lunar dates match the reference calendar and roll into the next month", () => {
	assert.match(day("2026-11-08").lunar, /九月三十$/);
	assert.equal(day("2026-11-08").label, "三十");
	assert.equal(day("2026-11-09").label, "十月");
	assert.match(day("2026-10-10").lunar, /九月初一$/);
	assert.match(day("2023-03-22").lunar, /闰二月初一$/);
	assert.ok(!day("2023-03-23").observances.includes("龙抬头"));
});

test("traditional festivals follow lunar dates rather than fixed Gregorian dates", () => {
	for (const [date, name] of [
		["2026-02-16", "除夕"],
		["2026-02-17", "春节"],
		["2026-06-19", "端午节"],
		["2026-09-25", "中秋节"],
		["2027-02-05", "除夕"],
		["2027-02-06", "春节"],
	]) {
		assert.ok(day(date).observances.includes(name), `${date}: ${name}`);
	}
});

test("2026 official holiday and make-up work boundaries match the published schedule", () => {
	for (const date of [
		"2026-01-01",
		"2026-01-03",
		"2026-02-15",
		"2026-02-23",
		"2026-09-25",
		"2026-09-27",
		"2026-10-01",
		"2026-10-07",
	])
		assert.equal(day(date).status, "rest", date);
	for (const date of [
		"2026-01-04",
		"2026-02-14",
		"2026-02-28",
		"2026-05-09",
		"2026-09-20",
		"2026-10-10",
	])
		assert.equal(day(date).status, "work", date);
	assert.equal(day("2026-10-08").status, undefined);
	assert.equal(day("2026-11-08").status, undefined);
});

test("2027 one-day holidays do not invent adjacent rest days or future schedules", () => {
	assert.equal(day("2027-06-09").status, "rest");
	assert.equal(day("2027-06-10").status, undefined);
	assert.equal(day("2027-09-15").status, "rest");
	assert.equal(day("2027-09-16").status, undefined);
	assert.equal(day("2027-09-26").status, "work");
	assert.equal(day("2027-10-09").status, "work");
	assert.equal(day("2028-10-01").status, undefined);
	assert.ok(day("2028-10-01").observances.includes("国庆节"));
});
