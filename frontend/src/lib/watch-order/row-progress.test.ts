import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { m } from '$lib/paraglide/messages';
import { progressOf, progressShare } from './entry';
import { captionParts, rowProgress } from './row-progress';
import { entryArb } from './testing/arbitraries';
import type { EntryProgress, WatchEntry } from './types';

const entry = (episodes: number | null): WatchEntry => ({
	id: '1',
	title: 'Saga',
	subtype: 'TV',
	start: '2010-01-01',
	episodes,
	status: episodes === null ? 'current' : 'finished',
	poster: null,
	cover: null
});

function row(e: WatchEntry, progress: EntryProgress | undefined) {
	return { entry: e, progress, share: progressShare(progressOf(e, progress)) };
}

describe('a franchise row’s progress', () => {
	it('captions a title part-way through, with the line filled to the share watched', () => {
		expect(rowProgress(row(entry(14), 6))).toEqual({
			kind: 'partway',
			caption: m.watchorder_meta_progress({ episode: 6, total: 14 }),
			share: 6 / 14
		});
	});

	it('says a finished title is watched, with no line', () => {
		expect(rowProgress(row(entry(14), 'watched'))).toEqual({
			kind: 'watched',
			caption: m.watchorder_meta_watched()
		});
	});

	it('captions a title with no known total by its episode alone, with no line', () => {
		expect(rowProgress(row(entry(null), 5))).toEqual({
			kind: 'open',
			caption: m.watchorder_meta_progress_open({ episode: 5 })
		});
	});

	it('shows nothing for a title not started', () => {
		expect(rowProgress(row(entry(14), undefined))).toEqual({ kind: 'none' });
	});

	it('draws a line only where it measures a title part-way through a known total', () => {
		const progress = fc.option(
			fc.oneof(fc.constant<'watched'>('watched'), fc.integer({ min: 1, max: 40 })),
			{ nil: undefined }
		);
		fc.assert(
			fc.property(entryArb('1'), progress, (e, p) => {
				const view = rowProgress(row(e, p));
				const measured = typeof p === 'number' && e.episodes !== null;
				expect(view.kind === 'partway').toBe(measured);
				if (view.kind === 'partway') {
					expect(view.share).toBeGreaterThan(0);
					expect(view.share).toBeLessThanOrEqual(1);
				}
				expect(view.kind === 'none').toBe(p === undefined);
			})
		);
	});
});

describe('a caption’s runs', () => {
	it('sets the digits apart from the words', () => {
		expect(captionParts('EP 6 of 14')).toEqual([
			{ text: 'EP ', digits: false },
			{ text: '6', digits: true },
			{ text: ' of ', digits: false },
			{ text: '14', digits: true }
		]);
		expect(captionParts('Watched')).toEqual([{ text: 'Watched', digits: false }]);
		expect(captionParts('12')).toEqual([{ text: '12', digits: true }]);
	});

	it('keeps the caption whole, in order, with no empty run', () => {
		fc.assert(
			fc.property(fc.string({ unit: fc.constantFrom('E', 'P', ' ', 'd', '1', '4', '0') }), (s) => {
				const parts = captionParts(s);
				expect(parts.map((p) => p.text).join('')).toBe(s);
				for (const p of parts) {
					expect(p.text).not.toBe('');
					expect(/^\d+$/.test(p.text)).toBe(p.digits);
				}
				for (let i = 1; i < parts.length; i++)
					expect(parts[i].digits).not.toBe(parts[i - 1].digits);
			})
		);
	});
});
