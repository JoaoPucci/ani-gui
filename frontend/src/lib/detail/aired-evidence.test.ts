import { describe, expect, it } from 'vitest';
import * as fc from 'fast-check';

import { datedAired, withAiredFloor, type DatedEpisode } from './aired-evidence';
import { epAirState, type AiringStatus } from './episode-airing';
import { airedCap } from './episode-caps';

function row(number: number | null, airdate: string | null): DatedEpisode {
	return { number, relative_number: null, airdate };
}

const at = (iso: string) => Date.parse(iso);

// Steel Ball Run as the anime database lists it: one twelve-episode
// entry whose first episode came out in March and whose second and
// third followed weekly from late September.
const STEEL_BALL_RUN = [row(1, '2026-03-19'), row(2, '2026-09-25'), row(3, '2026-10-02')];

// What the schedule says about the same show. The schedule's source
// files the March release as an entry of its own — one episode,
// finished — and that entry is the one the show maps to, so the count
// stops at one and nothing further is scheduled.
const ONE_FINISHED_PART: AiringStatus = {
	aired: 1,
	next_episode: null,
	next_airing_at: null,
	upcoming: []
};

describe('datedAired', () => {
	it('counts up to the latest episode whose air date is over', () => {
		expect(datedAired([STEEL_BALL_RUN], at('2026-10-04T17:26:00Z'))).toBe(3);
	});

	it('does not count an episode on its own air date', () => {
		// A date carries no time, and the schedule knows the hour. On
		// the day itself the schedule is the better witness, so the date
		// only speaks once the day is over everywhere a weekly slot can
		// fall — the UTC day covers a Japanese broadcast day whole,
		// late-night slots included.
		expect(datedAired([STEEL_BALL_RUN], at('2026-10-02T23:59:59Z'))).toBe(2);
		expect(datedAired([STEEL_BALL_RUN], at('2026-10-03T00:00:00Z'))).toBe(3);
	});

	it('ignores episodes dated in the future', () => {
		const rows = [row(8, '2026-09-27'), row(9, '2026-10-11'), row(10, '2026-10-18')];
		expect(datedAired([rows], at('2026-10-04T12:00:00Z'))).toBe(8);
	});

	it('ignores rows with no date, or with one it cannot read', () => {
		const rows = [row(1, '2026-03-19'), row(2, null), row(3, 'TBA'), row(4, '2026-3-9')];
		expect(datedAired([rows], at('2026-10-04T12:00:00Z'))).toBe(1);
	});

	it('reads through a gap: a later dated episode proves the ones before it', () => {
		const rows = [row(1, '2026-03-19'), row(2, null), row(3, '2026-10-02')];
		expect(datedAired([rows], at('2026-10-04T12:00:00Z'))).toBe(3);
	});

	it('takes the highest across every page it is given', () => {
		const first = [row(1, '2026-01-04'), row(2, '2026-01-11')];
		const second = [row(21, '2026-05-24'), row(22, '2026-05-31')];
		expect(datedAired([first, second], at('2026-10-04T12:00:00Z'))).toBe(22);
	});

	it('numbers a row the way the tiles do', () => {
		const relative: DatedEpisode = { number: null, relative_number: 2, airdate: '2026-09-25' };
		const unnumbered: DatedEpisode = { number: null, relative_number: null, airdate: '2026-09-25' };
		expect(datedAired([[relative, unnumbered]], at('2026-10-04T12:00:00Z'))).toBe(2);
	});

	it('counts whole episodes only', () => {
		// A recap filed as 2.5 says nothing about how many regular
		// episodes are out; zero and negatives are not episodes.
		const rows = [row(2.5, '2026-09-28'), row(0, '2026-03-01'), row(-1, '2026-03-01')];
		expect(datedAired([rows], at('2026-10-04T12:00:00Z'))).toBe(0);
	});

	it('is zero when there is nothing to read', () => {
		expect(datedAired([], at('2026-10-04T12:00:00Z'))).toBe(0);
		expect(datedAired([[]], at('2026-10-04T12:00:00Z'))).toBe(0);
	});
});

describe('withAiredFloor', () => {
	it('raises a schedule that stops short of what the dated episodes prove', () => {
		const airing = withAiredFloor(ONE_FINISHED_PART, 3);
		expect(airing?.aired).toBe(3);
		// The tiles follow: two and three are out, four is not.
		expect(epAirState(2, airing)).toEqual({ unaired: false });
		expect(epAirState(3, airing)).toEqual({ unaired: false });
		expect(epAirState(4, airing)).toEqual({ unaired: true, airsAt: null });
		// And so does every cap that clamps to the aired count.
		expect(airedCap(12, airing)).toBe(3);
	});

	it('leaves a schedule that already reaches the floor exactly as it was', () => {
		const weekly: AiringStatus = { aired: 8, next_episode: 9, next_airing_at: 1791532800 };
		expect(withAiredFloor(weekly, 8)).toBe(weekly);
		expect(withAiredFloor(weekly, 3)).toBe(weekly);
		expect(withAiredFloor(weekly, 0)).toBe(weekly);
	});

	it('never turns an unknown schedule into a known one', () => {
		// Unknown means nothing is gated. A floor would start gating
		// everything above it on evidence that only ever proves
		// episodes out, never episodes pending.
		const unknown: AiringStatus = { aired: null, next_episode: null, next_airing_at: null };
		expect(withAiredFloor(unknown, 3)).toBe(unknown);
		expect(withAiredFloor(null, 3)).toBeNull();
	});

	it('keeps a show that has not premiered gated when nothing is dated', () => {
		const unreleased: AiringStatus = { aired: 0, next_episode: 1, next_airing_at: 1796914800 };
		expect(withAiredFloor(unreleased, 0)).toBe(unreleased);
	});

	it('drops a next episode the floor has already passed', () => {
		const lagging: AiringStatus = {
			aired: 4,
			next_episode: 5,
			next_airing_at: 1791532800,
			upcoming: [
				{ episode: 5, airing_at: 1791532800 },
				{ episode: 6, airing_at: 1792137600 },
				{ episode: 7, airing_at: 1792742400 }
			]
		};
		// No tile may be both out and announced for a date.
		expect(withAiredFloor(lagging, 6)).toEqual({
			aired: 6,
			next_episode: null,
			next_airing_at: null,
			upcoming: [{ episode: 7, airing_at: 1792742400 }]
		});
	});

	it('keeps a next episode that is still ahead of the floor', () => {
		const ahead: AiringStatus = { aired: 1, next_episode: 5, next_airing_at: 1791532800 };
		expect(withAiredFloor(ahead, 3)).toEqual({
			aired: 3,
			next_episode: 5,
			next_airing_at: 1791532800
		});
	});

	it('does not invent a schedule list the answer never carried', () => {
		const legacy: AiringStatus = { aired: 1, next_episode: null, next_airing_at: null };
		expect(withAiredFloor(legacy, 2)).not.toHaveProperty('upcoming');
	});
});

describe('aired evidence — properties', () => {
	const isoDate = fc
		.date({ min: new Date('2000-01-01T00:00:00Z'), max: new Date('2099-12-31T00:00:00Z') })
		.filter((d) => !Number.isNaN(d.getTime()))
		.map((d) => d.toISOString().slice(0, 10));
	const datedRow = fc.record({
		number: fc.option(fc.integer({ min: -2, max: 2000 }), { nil: null }),
		relative_number: fc.option(fc.integer({ min: -2, max: 2000 }), { nil: null }),
		airdate: fc.option(fc.oneof(isoDate, fc.string()), { nil: null })
	});
	const pages = fc.array(fc.array(datedRow, { maxLength: 20 }), { maxLength: 4 });
	const moment = fc.integer({ min: at('2000-01-01T00:00:00Z'), max: at('2099-12-31T00:00:00Z') });
	const schedule = fc.record({
		aired: fc.option(fc.integer({ min: 0, max: 2000 }), { nil: null }),
		next_episode: fc.option(fc.integer({ min: 1, max: 2001 }), { nil: null }),
		next_airing_at: fc.option(fc.integer({ min: 0, max: 4102444800 }), { nil: null }),
		upcoming: fc.array(
			fc.record({
				episode: fc.integer({ min: 1, max: 2001 }),
				airing_at: fc.integer({ min: 0, max: 4102444800 })
			}),
			{ maxLength: 6 }
		)
	});

	it('time only ever adds to what the dates prove', () => {
		fc.assert(
			fc.property(
				pages,
				moment,
				fc.integer({ min: 0, max: 10 * 365 * 86_400_000 }),
				(p, t, later) => {
					expect(datedAired(p, t + later)).toBeGreaterThanOrEqual(datedAired(p, t));
				}
			)
		);
	});

	it('another page only ever adds to what the dates prove', () => {
		fc.assert(
			fc.property(pages, fc.array(datedRow, { maxLength: 20 }), moment, (p, extra, t) => {
				expect(datedAired([...p, extra], t)).toBeGreaterThanOrEqual(datedAired(p, t));
			})
		);
	});

	it('a floor never lowers the aired count, and never un-airs an episode', () => {
		fc.assert(
			fc.property(schedule, fc.integer({ min: 0, max: 2000 }), (s, floor) => {
				const out = withAiredFloor(s, floor);
				if (s.aired === null) {
					expect(out).toBe(s);
					return;
				}
				if (floor <= s.aired) {
					// Nothing to raise: the schedule stands as it came,
					// whatever it says about what is next.
					expect(out).toBe(s);
					return;
				}
				expect(out?.aired).toBe(floor);
				for (const u of out?.upcoming ?? []) {
					expect(u.episode).toBeGreaterThan(floor);
				}
				if (out?.next_episode != null) {
					expect(out.next_episode).toBeGreaterThan(floor);
				}
			})
		);
	});

	it('applying the same floor twice changes nothing more', () => {
		fc.assert(
			fc.property(schedule, fc.integer({ min: 0, max: 2000 }), (s, floor) => {
				const once = withAiredFloor(s, floor);
				expect(withAiredFloor(once, floor)).toBe(once);
			})
		);
	});
});
