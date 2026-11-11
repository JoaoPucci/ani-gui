import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { isUnaired, localDay, progressOf, progressShare, yearOf } from './entry';
import { FIXTURE_RUNS, type FixtureRun } from './fixtures/runs';
import type { WatchEntry } from './types';

const TODAY = '2026-11-02';
const run = (key: string): FixtureRun => FIXTURE_RUNS.find((r) => r.key === key)!;
function entryIn(r: FixtureRun, id: string): WatchEntry {
	return [...r.entries, ...r.versions, ...r.films, ...r.sides, ...r.recaps].find(
		(e) => e.id === id
	)!;
}

describe('entry readings', () => {
	it('reads progress as none, watched or part-way', () => {
		const jojo = run('jojo');
		const at = (id: string) => progressOf(entryIn(jojo, id), jojo.progress[id]);
		expect(at('610')).toEqual({ kind: 'none' });
		expect(at('7158')).toEqual({ kind: 'watched' });
		expect(at('46598')).toEqual({ kind: 'partway', episode: 13, total: 14 });
		expect(progressShare(at('7158'))).toBe(1);
		expect(progressShare(at('610'))).toBe(0);
		expect(progressShare({ kind: 'partway', episode: 3, total: null })).toBe(0);
		expect(progressShare({ kind: 'partway', episode: 7, total: 14 })).toBe(0.5);
	});

	it('knows aired from unaired', () => {
		const mono = run('monogatari');
		expect(isUnaired(entryIn(mono, '50013'), TODAY)).toBe(true);
		expect(isUnaired(entryIn(mono, '3919'), TODAY)).toBe(false);
		expect(yearOf(entryIn(mono, '3919'))).toBe('2009');
		expect(yearOf(entryIn(mono, '50013'))).toBeNull();
	});
});

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
