import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import {
	ageOf,
	nextAgeChange,
	statusLabel,
	statusPhrases,
	watchLaterStatus,
	type Age,
	type StatusCopy
} from './watch-later-status';

const NOW = 1_800_000_000_000;
const YEAR = 365 * 24 * 60 * 60 * 1000;

const stamp = fc.integer({ min: NOW - YEAR, max: NOW + 60 * 60 * 1000 });
const maybeStamp = fc.option(stamp, { nil: null });
const elapsed = fc.integer({ min: -60 * 60 * 1000, max: YEAR });

const RANK: Record<Age['unit'], number> = { now: 0, minute: 1, hour: 2, day: 3 };
/** A total order over ages: unit first, then count. */
const order = (a: Age) => RANK[a.unit] * 1e9 + ('count' in a ? a.count : 0);

describe('watchLaterStatus, by property', () => {
	it('is null exactly when no tracker is connected', () => {
		fc.assert(
			fc.property(fc.array(maybeStamp, { maxLength: 4 }), fc.boolean(), (stamps, refreshing) => {
				expect(watchLaterStatus(stamps, refreshing) === null).toBe(stamps.length === 0);
			})
		);
	});

	it('reports the oldest stamp when every tracker has one, regardless of their order', () => {
		fc.assert(
			fc.property(fc.array(stamp, { minLength: 1, maxLength: 4 }), (stamps) => {
				const status = watchLaterStatus(stamps, false);
				expect(status).toEqual({ kind: 'updated', at: Math.min(...stamps) });
				expect(watchLaterStatus([...stamps].reverse(), false)).toEqual(status);
			})
		);
	});

	it('is never once any tracker lacks a stamp, unless a pull is under way', () => {
		fc.assert(
			fc.property(
				fc.array(maybeStamp, { minLength: 1, maxLength: 4 }),
				fc.boolean(),
				(stamps, refreshing) => {
					const status = watchLaterStatus(stamps, refreshing);
					if (refreshing) expect(status).toEqual({ kind: 'refreshing' });
					else if (stamps.includes(null)) expect(status).toEqual({ kind: 'never' });
					else expect(status?.kind).toBe('updated');
				}
			)
		);
	});
});

describe('ageOf, by property', () => {
	it('never runs backwards as the clock moves forward', () => {
		fc.assert(
			fc.property(elapsed, fc.integer({ min: 0, max: YEAR }), (e, later) => {
				const at = NOW - e;
				expect(order(ageOf(at, NOW + later))).toBeGreaterThanOrEqual(order(ageOf(at, NOW)));
			})
		);
	});

	it('counts at least one of its unit', () => {
		fc.assert(
			fc.property(elapsed, (e) => {
				const age = ageOf(NOW - e, NOW);
				if (age.unit !== 'now') expect(age.count).toBeGreaterThanOrEqual(1);
			})
		);
	});
});

describe('nextAgeChange, by property', () => {
	it('is positive, holds the age until then, and changes it exactly then', () => {
		fc.assert(
			fc.property(elapsed, (e) => {
				const at = NOW - e;
				const wait = nextAgeChange(at, NOW);
				expect(wait).toBeGreaterThan(0);
				expect(ageOf(at, NOW + wait - 1)).toEqual(ageOf(at, NOW));
				expect(ageOf(at, NOW + wait)).not.toEqual(ageOf(at, NOW));
			})
		);
	});

	// The wait goes straight to setTimeout, which wraps a delay past
	// 2^31 − 1 ms (about 24.8 days) to nearly nothing: the page would
	// re-render in a loop for as long as a stamp sat that far ahead of
	// the clock (a pull made while the clock ran fast, then corrected).
	it('never asks for a wait a timer cannot hold, however far the clock is skewed', () => {
		fc.assert(
			fc.property(fc.integer({ min: -10 * YEAR, max: YEAR }), (e) => {
				const at = NOW - e;
				const wait = nextAgeChange(at, NOW);
				expect(wait).toBeGreaterThan(0);
				expect(wait).toBeLessThanOrEqual(2 ** 31 - 1);
				expect(ageOf(at, NOW + wait - 1)).toEqual(ageOf(at, NOW));
			})
		);
	});
});

describe('statusPhrases, by property', () => {
	const copy: StatusCopy = {
		refreshing: () => 'Refreshing…',
		never: () => 'Not updated yet',
		justNow: () => 'Updated just now',
		updated: (when) => `Updated ${when}`
	};
	const status = fc.oneof(
		fc.constant({ kind: 'refreshing' as const }),
		fc.constant({ kind: 'never' as const }),
		stamp.map((at) => ({ kind: 'updated' as const, at }))
	);
	const held = fc.option(fc.constantFrom('Updated 12 min. ago', 'Not updated yet', 'Refreshing…'), {
		nil: null
	});
	it('shows exactly the label, once, and holds at most the one phrasing it was given', () => {
		fc.assert(
			fc.property(status, fc.constantFrom('en', 'es-419', 'pt-BR', 'ru'), held, (s, locale, h) => {
				const phrases = statusPhrases(s, NOW, locale, copy, h);
				expect(phrases.filter((p) => p.shown).map((p) => p.text)).toEqual([
					statusLabel(s, NOW, locale, copy)
				]);
				const hidden = phrases.filter((p) => !p.shown).map((p) => p.text);
				expect(hidden.length).toBeLessThanOrEqual(1);
				if (hidden.length === 1) expect(hidden[0]).toBe(h);
			})
		);
	});
});
