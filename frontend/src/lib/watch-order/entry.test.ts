import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { localDay } from './entry';

describe('localDay', () => {
	it("is the viewer's own calendar day, not UTC's", () => {
		// Half past midnight and a minute to midnight, local time.
		expect(localDay(new Date(2027, 0, 10, 0, 30))).toBe('2027-01-10');
		expect(localDay(new Date(2026, 11, 31, 23, 59))).toBe('2026-12-31');
	});

	it('pads to YYYY-MM-DD, the form start dates are compared in', () => {
		fc.assert(
			fc.property(
				fc.date({ min: new Date(1990, 0, 1), max: new Date(2090, 0, 1), noInvalidDate: true }),
				(d) => {
					const day = localDay(d);
					expect(day).toMatch(/^\d{4}-\d{2}-\d{2}$/);
					const [y, mo, da] = day.split('-').map(Number);
					expect([y, mo - 1, da]).toEqual([d.getFullYear(), d.getMonth(), d.getDate()]);
				}
			)
		);
	});
});
