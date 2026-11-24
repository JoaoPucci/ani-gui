import { afterEach, describe, expect, it } from 'vitest';
import { getLocale, overwriteGetLocale } from '$lib/paraglide/runtime';
import type { ProgressView, ViewEntry } from '$lib/api';
import { loopGroupText, metaLine, numeral, splitMeta, waysKeyText } from './meta';

/** An entry as the backend answers it: aired, and not started, unless said. */
function entry(over: Partial<ViewEntry> = {}): ViewEntry {
	return {
		id: '1',
		title: 'Saga',
		subtype: 'TV',
		start: '2024-01-10',
		episodes: 14,
		status: 'finished',
		poster: null,
		cover: null,
		unaired: false,
		progress: { kind: 'none' },
		...over
	};
}
const partway = (episode: number, total: number | null): ProgressView => ({
	kind: 'partway',
	episode,
	total
});
const line = (e: ViewEntry, withProgress?: boolean) =>
	metaLine(e, withProgress).map((s) => `${s.kind}:${s.text}`);

describe('meta line', () => {
	it('shows format, year and episode count for an unstarted entry', () => {
		expect(line(entry())).toEqual(['word:TV', 'num:2024', 'num:14', 'unit:ep']);
	});

	it('shows progress or watched in place of the count', () => {
		expect(line(entry({ start: '2022-01-01', progress: partway(13, 14) }))).toEqual([
			'word:TV',
			'num:2022',
			'num:EP 13 of 14'
		]);
		expect(line(entry({ start: '2012-10-06', progress: { kind: 'watched' } }))).toEqual([
			'word:TV',
			'num:2012',
			'watched:Watched'
		]);
	});

	it('leaves the progress out where the card shows it elsewhere, giving the count', () => {
		expect(line(entry({ progress: partway(13, 14) }), false)).toEqual([
			'word:TV',
			'num:2024',
			'num:14',
			'unit:ep'
		]);
		expect(line(entry({ progress: { kind: 'watched' } }), false)).toEqual([
			'word:TV',
			'num:2024',
			'num:14',
			'unit:ep'
		]);
	});

	it('leaves the count off a single video', () => {
		expect(line(entry({ subtype: 'movie', start: '2021-01-08', episodes: 1 }))).toEqual([
			'word:MOVIE',
			'num:2021'
		]);
	});

	it('dates an unaired entry, or says no date is announced', () => {
		expect(line(entry({ subtype: 'special', start: null, unaired: true }))).toEqual([
			'word:SPECIAL',
			'word:Date not announced'
		]);
		expect(line(entry({ start: '2026-07-01', unaired: true }))).toEqual([
			'word:TV',
			'word:Coming July 2026'
		]);
	});

	describe('in Russian', () => {
		const initialLocale = getLocale();
		afterEach(() => overwriteGetLocale(() => initialLocale));

		// The month and year come from Intl in the nominative («июль
		// 2026 г.»), which «Выходит …» would need in another case.
		it('dates an unaired entry with a label the nominative date fits', () => {
			overwriteGetLocale(() => 'ru');
			expect(line(entry({ start: '2026-07-01', unaired: true }))).toEqual([
				'word:ТВ',
				'word:Выход: июль 2026 г.'
			]);
		});
	});

	it('reads progress without a total as the episode alone', () => {
		expect(metaLine(entry({ episodes: null, progress: partway(3, null) })).at(-1)).toEqual({
			kind: 'num',
			text: 'EP 3'
		});
	});

	it('gives no year for an entry with no start that has aired', () => {
		expect(line(entry({ start: null }))).toEqual(['word:TV', 'num:14', 'unit:ep']);
	});

	it('pads positions to two digits', () => {
		expect(numeral(5)).toBe('05');
		expect(numeral(16)).toBe('16');
	});
});

describe('splitMeta', () => {
	const parts = (e: ViewEntry) => splitMeta(metaLine(e)).map((p) => p.map((s) => s.text));

	it('puts the format and year first, and the rest on its own line', () => {
		expect(parts(entry({ start: '2022-01-01', progress: partway(13, 14) }))).toEqual([
			['TV', '2022'],
			['EP 13 of 14']
		]);
		expect(parts(entry({ start: '2012-10-06', progress: { kind: 'watched' } }))).toEqual([
			['TV', '2012'],
			['Watched']
		]);
		expect(parts(entry())).toEqual([
			['TV', '2024'],
			['14', 'ep']
		]);
	});

	it('keeps a line with nothing after the year whole', () => {
		expect(parts(entry({ subtype: 'movie', start: '2021-01-08', episodes: 1 }))).toEqual([
			['MOVIE', '2021'],
			[]
		]);
	});

	it("puts an unaired entry's date on the second line", () => {
		expect(parts(entry({ subtype: 'special', start: null, unaired: true }))).toEqual([
			['SPECIAL'],
			['Date not announced']
		]);
	});
});

describe('waysKeyText', () => {
	it('counts the ways on, except for a franchise read in part', () => {
		expect(waysKeyText(2, true)).toBe('2 ways to continue');
		expect(waysKeyText(2, false)).toBe('Ways to continue');
	});
});

describe('loopGroupText', () => {
	it('heads a loop as the group of titles it is', () => {
		expect(loopGroupText(10, true)).toBe('10 linked titles, order unclear');
		expect(loopGroupText(2, true)).toBe('2 linked titles, order unclear');
	});

	it('gives no count for a franchise read in part, where it may be short', () => {
		expect(loopGroupText(10, false)).toBe('Linked titles, order unclear');
		expect(loopGroupText(2, false)).toBe('Linked titles, order unclear');
	});
});
