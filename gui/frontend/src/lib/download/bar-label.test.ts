/**
 * Tests for the DownloadBar's adaptive label helper.
 *
 * The new bar is a single status row (Android Studio status bar
 * style); the label shape changes with active-download count.
 * Returns a discriminated union so the Svelte template picks the
 * right Paraglide message at render time and the helper stays
 * dependency-free for unit tests.
 *
 *   - 0 active → null (caller hides the bar)
 *   - 1 active, no range → { kind: 'single', title, episode }
 *   - 1 active, range with currentEp → { kind: 'single-range-progress',
 *     title, currentEp, total }
 *   - 1 active, range without currentEp → { kind: 'single-range-static',
 *     title, episode }
 *   - N > 1 → { kind: 'multi', episodeCount }
 *     where episodeCount is `sum(rangeTotal ?? 1)` across items
 */

import { describe, expect, it } from 'vitest';
import { formatDownloadBarLabel, type DownloadBarItem } from './bar-label';

function item(overrides: Partial<DownloadBarItem> = {}): DownloadBarItem {
	return {
		title: 'Test Show',
		episode: '1',
		rangeTotal: null,
		currentEp: null,
		...overrides
	};
}

describe('formatDownloadBarLabel', () => {
	it('returns null for an empty list', () => {
		expect(formatDownloadBarLabel([])).toBeNull();
	});

	it('classifies a single non-range item as kind:single', () => {
		const out = formatDownloadBarLabel([item({ title: 'Naruto', episode: '5' })]);
		expect(out).toEqual({ kind: 'single', title: 'Naruto', episode: '5' });
	});

	it('classifies a single range item with known currentEp as kind:single-range-progress', () => {
		const out = formatDownloadBarLabel([
			item({ title: 'Bleach', episode: '1-12', rangeTotal: 12, currentEp: 3 })
		]);
		expect(out).toEqual({
			kind: 'single-range-progress',
			title: 'Bleach',
			currentEp: 3,
			total: 12
		});
	});

	it('falls back to kind:single-range-static when range exists but currentEp is null', () => {
		const out = formatDownloadBarLabel([
			item({ title: 'Bleach', episode: '1-12', rangeTotal: 12, currentEp: null })
		]);
		expect(out).toEqual({
			kind: 'single-range-static',
			title: 'Bleach',
			episode: '1-12'
		});
	});

	it('classifies multi-item as kind:multi with summed episode count', () => {
		const out = formatDownloadBarLabel([
			item({ title: 'A', episode: '1' }),
			item({ title: 'B', episode: '1-3', rangeTotal: 3 }),
			item({ title: 'C', episode: '4' })
		]);
		// 1 + 3 + 1 = 5
		expect(out).toEqual({ kind: 'multi', episodeCount: 5 });
	});

	it('treats null rangeTotal as a single episode in the sum', () => {
		const out = formatDownloadBarLabel([item({ episode: '1' }), item({ episode: '2' })]);
		expect(out).toEqual({ kind: 'multi', episodeCount: 2 });
	});
});
