import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import type { ProgressView, ViewEntry } from '$lib/api';
import { m } from '$lib/paraglide/messages';
import { captionParts, rowProgress } from './row-progress';

const entry = (episodes: number | null, progress: ProgressView): ViewEntry => ({
	id: '1',
	title: 'Saga',
	subtype: 'TV',
	start: '2010-01-01',
	episodes,
	status: episodes === null ? 'current' : 'finished',
	poster: null,
	cover: null,
	unaired: false,
	progress
});

/** A row as the backend answers it: the entry's progress, and its share. */
function row(episodes: number | null, progress: ProgressView, share = 0) {
	return { entry: entry(episodes, progress), share };
}

describe('a franchise row’s progress', () => {
	it('captions a title part-way through, with the line filled to the share watched', () => {
		expect(rowProgress(row(14, { kind: 'partway', episode: 6, total: 14 }, 6 / 14))).toEqual({
			kind: 'partway',
			caption: m.watchorder_meta_progress({ episode: 6, total: 14 }),
			share: 6 / 14
		});
	});

	it('says a finished title is watched, with no line', () => {
		expect(rowProgress(row(14, { kind: 'watched' }, 1))).toEqual({
			kind: 'watched',
			caption: m.watchorder_meta_watched()
		});
	});

	it('captions a title with no known total by its episode alone, with no line', () => {
		expect(rowProgress(row(null, { kind: 'partway', episode: 5, total: null }))).toEqual({
			kind: 'open',
			caption: m.watchorder_meta_progress_open({ episode: 5 })
		});
	});

	it('shows nothing for a title not started', () => {
		expect(rowProgress(row(14, { kind: 'none' }))).toEqual({ kind: 'none' });
	});

	it('draws a line only where it measures a title part-way through a known total', () => {
		const progress = fc.oneof(
			fc.constant<ProgressView>({ kind: 'none' }),
			fc.constant<ProgressView>({ kind: 'watched' }),
			fc.record({
				kind: fc.constant<'partway'>('partway'),
				episode: fc.integer({ min: 1, max: 40 }),
				total: fc.option(fc.integer({ min: 1, max: 40 }), { nil: null })
			})
		);
		const share = fc.double({ min: 0, max: 1, noNaN: true });
		fc.assert(
			fc.property(progress, share, (p, sh) => {
				const view = rowProgress(row(p.kind === 'partway' ? p.total : 12, p, sh));
				const measured = p.kind === 'partway' && p.total !== null;
				expect(view.kind === 'partway').toBe(measured);
				if (view.kind === 'partway') expect(view.share).toBe(sh);
				expect(view.kind === 'none').toBe(p.kind === 'none');
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
