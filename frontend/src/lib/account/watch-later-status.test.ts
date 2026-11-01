import { describe, expect, it } from 'vitest';

import {
	ageOf,
	nextAgeChange,
	statusLabel,
	statusPhrases,
	watchLaterStatus,
	type StatusCopy
} from './watch-later-status';

const S = 1_000;
const MIN = 60 * S;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;
const NOW = 1_800_000_000_000;

const copy: StatusCopy = {
	refreshing: () => 'Refreshing…',
	never: () => 'Not updated yet',
	justNow: () => 'Updated just now',
	updated: (when) => `Updated ${when}`
};

describe('watchLaterStatus', () => {
	it('says nothing when no tracker is connected', () => {
		expect(watchLaterStatus([], false)).toBeNull();
		expect(watchLaterStatus([], true)).toBeNull();
	});

	it('reports a pull under way before anything else', () => {
		expect(watchLaterStatus([NOW], true)).toEqual({ kind: 'refreshing' });
		expect(watchLaterStatus([null], true)).toEqual({ kind: 'refreshing' });
	});

	it('is not yet updated while any connected tracker has no recorded pull', () => {
		expect(watchLaterStatus([null], false)).toEqual({ kind: 'never' });
		expect(watchLaterStatus([NOW - MIN, null], false)).toEqual({ kind: 'never' });
		expect(watchLaterStatus([null, NOW - MIN], false)).toEqual({ kind: 'never' });
	});

	it('is as fresh as the oldest tracker', () => {
		expect(watchLaterStatus([NOW - MIN], false)).toEqual({ kind: 'updated', at: NOW - MIN });
		expect(watchLaterStatus([NOW - MIN, NOW - 3 * HOUR], false)).toEqual({
			kind: 'updated',
			at: NOW - 3 * HOUR
		});
		expect(watchLaterStatus([NOW - 3 * HOUR, NOW - MIN], false)).toEqual({
			kind: 'updated',
			at: NOW - 3 * HOUR
		});
	});
});

describe('ageOf', () => {
	const cases: [number, ReturnType<typeof ageOf>][] = [
		[0, { unit: 'now' }],
		[MIN - 1, { unit: 'now' }],
		[MIN, { unit: 'minute', count: 1 }],
		[12 * MIN + 30 * S, { unit: 'minute', count: 12 }],
		[HOUR - 1, { unit: 'minute', count: 59 }],
		[HOUR, { unit: 'hour', count: 1 }],
		[DAY - 1, { unit: 'hour', count: 23 }],
		[DAY, { unit: 'day', count: 1 }],
		[9 * DAY + HOUR, { unit: 'day', count: 9 }]
	];
	for (const [elapsed, age] of cases) {
		it(`buckets ${elapsed} ms as ${JSON.stringify(age)}`, () => {
			expect(ageOf(NOW - elapsed, NOW)).toEqual(age);
		});
	}

	it('reads a time ahead of the clock (a skewed stamp) as just now', () => {
		expect(ageOf(NOW + 5 * MIN, NOW)).toEqual({ unit: 'now' });
	});
});

describe('nextAgeChange', () => {
	it('waits for the next whole minute while under an hour', () => {
		expect(nextAgeChange(NOW, NOW)).toBe(MIN);
		expect(nextAgeChange(NOW - 90 * S, NOW)).toBe(30 * S);
		expect(nextAgeChange(NOW - 59 * MIN, NOW)).toBe(MIN);
	});

	it('waits for the next whole hour, then the next whole day', () => {
		expect(nextAgeChange(NOW - (HOUR + 10 * MIN), NOW)).toBe(50 * MIN);
		expect(nextAgeChange(NOW - (DAY + HOUR), NOW)).toBe(23 * HOUR);
	});

	it('waits out a skewed stamp before the first minute starts', () => {
		expect(nextAgeChange(NOW + 5 * MIN, NOW)).toBe(6 * MIN);
	});
});

describe('statusLabel', () => {
	it('says nothing for no status', () => {
		expect(statusLabel(null, NOW, 'en', copy)).toBeNull();
	});

	it('names the refreshing and never states in the caller’s words', () => {
		expect(statusLabel({ kind: 'refreshing' }, NOW, 'en', copy)).toBe('Refreshing…');
		expect(statusLabel({ kind: 'never' }, NOW, 'en', copy)).toBe('Not updated yet');
	});

	it('says just now inside the first minute', () => {
		expect(statusLabel({ kind: 'updated', at: NOW - 20 * S }, NOW, 'en', copy)).toBe(
			'Updated just now'
		);
	});

	it('formats the age for the locale', () => {
		const at = NOW - 12 * MIN;
		const label = (locale: string) => statusLabel({ kind: 'updated', at }, NOW, locale, copy);
		const fmt = (locale: string) =>
			new Intl.RelativeTimeFormat(locale, { numeric: 'always', style: 'short' }).format(
				-12,
				'minute'
			);
		for (const locale of ['en', 'es-419', 'pt-BR', 'ru']) {
			expect(label(locale), locale).toBe(`Updated ${fmt(locale)}`);
		}
		expect(label('en')).toContain('12');
		expect(label('ru')).toContain('12');
	});

	it('formats hours and days', () => {
		expect(statusLabel({ kind: 'updated', at: NOW - 3 * HOUR }, NOW, 'en', copy)).toBe(
			`Updated ${new Intl.RelativeTimeFormat('en', { numeric: 'always', style: 'short' }).format(-3, 'hour')}`
		);
		expect(statusLabel({ kind: 'updated', at: NOW - 2 * DAY }, NOW, 'en', copy)).toBe(
			`Updated ${new Intl.RelativeTimeFormat('en', { numeric: 'always', style: 'short' }).format(-2, 'day')}`
		);
	});
});

describe('statusPhrases', () => {
	it('has none for no status', () => {
		expect(statusPhrases(null, NOW, 'en', copy)).toEqual([]);
	});

	it('shows the current label and holds every other phrasing, once each', () => {
		const phrases = statusPhrases({ kind: 'updated', at: NOW - 12 * MIN }, NOW, 'en', copy);
		const shown = phrases.filter((p) => p.shown).map((p) => p.text);
		expect(shown).toEqual([statusLabel({ kind: 'updated', at: NOW - 12 * MIN }, NOW, 'en', copy)]);
		const texts = phrases.map((p) => p.text);
		expect(texts).toContain('Refreshing…');
		expect(texts).toContain('Not updated yet');
		expect(texts).toContain('Updated just now');
		expect(texts.some((t) => t.includes('59'))).toBe(true);
		expect(new Set(texts).size).toBe(texts.length);
	});

	it('shows the refreshing phrasing while a pull runs', () => {
		const phrases = statusPhrases({ kind: 'refreshing' }, NOW, 'en', copy);
		expect(phrases.filter((p) => p.shown).map((p) => p.text)).toEqual(['Refreshing…']);
	});
});
