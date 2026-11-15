import { describe, expect, it } from 'vitest';
import { SEARCH_TYPE_FILTERS } from './type-filters';
import { LABELLED_SUBTYPES } from './subtype-label';
import { isMusicSubtype } from '$lib/history/resolve';

describe('SEARCH_TYPE_FILTERS', () => {
	it('offers no music filter: search never answers a music video', () => {
		expect(SEARCH_TYPE_FILTERS.filter(isMusicSubtype)).toEqual([]);
	});

	it('offers every labelled subtype search can answer, each once', () => {
		// The backend drops music videos from every search (drop_music),
		// so a music filter could only ever empty the grid. Every other
		// subtype with a label may come back, and has a filter.
		const offered = SEARCH_TYPE_FILTERS.map((s) => s.toLowerCase());
		const answerable = LABELLED_SUBTYPES.filter((s) => !isMusicSubtype(s));
		expect([...offered].sort()).toEqual([...answerable].sort());
		expect(new Set(offered).size).toBe(offered.length);
	});
});
