import { describe, expect, it } from 'vitest';

import {
	airedCap,
	airedTargets,
	epAirState,
	formatAirDate,
	type AiringStatus
} from './episode-airing';

// Yani Neko's real shape at the time of writing: 12 announced, 2
// aired, ep 3 lands 2026-07-17 00:30 JST (epoch 1784215800).
const RELEASING: AiringStatus = {
	aired: 2,
	next_episode: 3,
	next_airing_at: 1784215800
};

describe('epAirState', () => {
	it('keeps tiles at or below the aired count interactive', () => {
		expect(epAirState(1, RELEASING)).toEqual({ unaired: false });
		expect(epAirState(2, RELEASING)).toEqual({ unaired: false });
	});

	it('greys tiles above the aired count', () => {
		expect(epAirState(4, RELEASING)).toEqual({ unaired: true, airsAt: null });
		expect(epAirState(12, RELEASING)).toEqual({ unaired: true, airsAt: null });
	});

	it('carries the air date only on the very next episode', () => {
		expect(epAirState(3, RELEASING)).toEqual({ unaired: true, airsAt: 1784215800 });
	});

	it('never gates when airing data is unknown', () => {
		expect(epAirState(12, null)).toEqual({ unaired: false });
		expect(epAirState(12, { aired: null, next_episode: null, next_airing_at: null })).toEqual({
			unaired: false
		});
	});

	it('gates everything for a not-yet-premiered show', () => {
		const unreleased: AiringStatus = { aired: 0, next_episode: null, next_airing_at: null };
		expect(epAirState(1, unreleased)).toEqual({ unaired: true, airsAt: null });
	});

	it('labels every scheduled unaired episode with its own date', () => {
		// The backend now carries the published future schedule, so
		// tiles beyond the next episode get their dates too.
		const withSchedule: AiringStatus = {
			...RELEASING,
			upcoming: [
				{ episode: 3, airing_at: 1784215800 },
				{ episode: 4, airing_at: 1784820600 }
			]
		};
		expect(epAirState(3, withSchedule)).toEqual({ unaired: true, airsAt: 1784215800 });
		expect(epAirState(4, withSchedule)).toEqual({ unaired: true, airsAt: 1784820600 });
		// Past the published window: honestly dateless.
		expect(epAirState(12, withSchedule)).toEqual({ unaired: true, airsAt: null });
	});

	it('falls back to next_airing_at when the schedule is absent', () => {
		// RELEASING has no upcoming list — the next episode keeps its
		// date via the nextAiringEpisode fallback.
		expect(epAirState(3, RELEASING)).toEqual({ unaired: true, airsAt: 1784215800 });
	});

	it('keeps released decimal extras playable', () => {
		// Codex P2 #3565610386: allmanga exposes recaps/specials as
		// decimal tags (2.5 airs between regular eps 2 and 3). AniList
		// only counts regular episodes, so a strict n <= aired check
		// would grey a streamable 2.5 until ep 3 airs. Floor-compare:
		// the extra is out once its base episode is.
		expect(epAirState(2.5, RELEASING)).toEqual({ unaired: false });
		expect(airedTargets([1, 2, 2.5, 3], RELEASING)).toEqual([1, 2, 2.5]);
	});

	it('still gates decimal extras beyond the aired count', () => {
		expect(epAirState(3.5, RELEASING)).toEqual({ unaired: true, airsAt: null });
	});
});

describe('airedCap', () => {
	it('clamps the episode cap to the aired count', () => {
		// Codex P2 #3565649454: the primary Play/Continue CTA computes
		// its target from the cap; without the clamp a user watched
		// through the aired count gets "Continue" into an unaired
		// episode — the same doomed resolution the tiles disable.
		expect(airedCap(12, RELEASING)).toBe(2);
	});

	it('passes the cap through on unknown airing data', () => {
		expect(airedCap(12, null)).toBe(12);
		expect(airedCap(12, { aired: null, next_episode: null, next_airing_at: null })).toBe(12);
	});

	it('uses the aired count when no cap is known', () => {
		expect(airedCap(null, RELEASING)).toBe(2);
	});

	it('keeps a smaller cap over a larger aired count', () => {
		expect(airedCap(2, { ...RELEASING, aired: 10 })).toBe(2);
	});

	it('stays unbounded when neither is known', () => {
		expect(airedCap(null, null)).toBe(null);
	});

	it('collapses to zero for a not-yet-premiered show', () => {
		// Drives the primary CTA block (Codex P2 #3565666393): a
		// searchable allmanga stub with nothing aired must not offer
		// Play episode 1.
		expect(airedCap(12, { aired: 0, next_episode: null, next_airing_at: null })).toBe(0);
	});
});

describe('airedTargets', () => {
	it('drops unaired episode numbers from a prefetch list', () => {
		// Codex P2 #3565590966: the detail-page warm must not spend
		// scraper slots resolving greyed-out future episodes.
		expect(airedTargets([1, 2, 3, 4, 12], RELEASING)).toEqual([1, 2]);
	});

	it('passes everything through on unknown airing data', () => {
		expect(airedTargets([1, 2, 3], null)).toEqual([1, 2, 3]);
		expect(airedTargets([5], { aired: null, next_episode: null, next_airing_at: null })).toEqual([
			5
		]);
	});

	it('empties the list for a not-yet-premiered show', () => {
		expect(airedTargets([1, 2], { aired: 0, next_episode: null, next_airing_at: null })).toEqual(
			[]
		);
	});
});

describe('formatAirDate', () => {
	it('formats the epoch as a short localized date', () => {
		// 1784215800 = 2026-07-16T15:30:00Z; en-US in UTC renders Jul 16.
		// The helper uses the viewer's zone, so pin the assertion loosely:
		// month appears and it's a non-empty short string.
		const got = formatAirDate(1784215800, 'en-US');
		expect(got.length).toBeGreaterThan(0);
		expect(got).toMatch(/Jul/);
	});

	it('respects the locale', () => {
		const got = formatAirDate(1784215800, 'ru');
		expect(got.length).toBeGreaterThan(0);
		expect(got).not.toMatch(/Jul/); // ru renders "июл", not "Jul"
	});
});
