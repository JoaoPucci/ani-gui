import { describe, expect, it } from 'vitest';

import { airedTargets, epAirState, formatAirDate, type AiringStatus } from './episode-airing';

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
