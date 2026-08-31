import { describe, expect, it } from 'vitest';
import { decideStripFollow } from './strip-follow';

describe('decideStripFollow', () => {
	it('follows a next-press off the end of the visible page', () => {
		// The reported bug: watching ep 5 with the strip on page 1
		// (eps 1–5), Next lands on ep 6 but the strip kept showing
		// the stale page.
		expect(decideStripFollow({ prevEpisode: 5, episode: 6, currentPage: 1, pageSize: 5 })).toEqual({
			follow: true,
			page: 2
		});
	});

	it('follows a prev-press back across the page boundary', () => {
		// Symmetric: first tile of page 2 → Prev → last tile of page 1.
		expect(decideStripFollow({ prevEpisode: 6, episode: 5, currentPage: 2, pageSize: 5 })).toEqual({
			follow: true,
			page: 1
		});
	});

	it('follows a multi-page jump when the strip was tracking playback', () => {
		expect(decideStripFollow({ prevEpisode: 5, episode: 23, currentPage: 1, pageSize: 5 })).toEqual(
			{ follow: true, page: 5 }
		);
	});

	it('stays put when the new episode is already on the visible page', () => {
		expect(decideStripFollow({ prevEpisode: 3, episode: 4, currentPage: 1, pageSize: 5 })).toEqual({
			follow: false
		});
	});

	it('stays put when the user has paginated away to browse', () => {
		// Auto-play advancing 12 → 13 must not yank a strip the user
		// deliberately moved to page 7 — carousels respond to the
		// user, not to timers.
		expect(
			decideStripFollow({ prevEpisode: 12, episode: 13, currentPage: 7, pageSize: 5 })
		).toEqual({ follow: false });
	});

	it('stays put when the browsed page already contains the new episode', () => {
		// User paginated ahead and clicked the tile they were looking
		// at — the strip is already right.
		expect(decideStripFollow({ prevEpisode: 5, episode: 6, currentPage: 2, pageSize: 5 })).toEqual({
			follow: false
		});
	});

	it('stays put on a same-episode re-landing', () => {
		// Stale-stream recovery re-resolves the current episode; the
		// user may be browsing another page meanwhile.
		expect(decideStripFollow({ prevEpisode: 6, episode: 6, currentPage: 3, pageSize: 5 })).toEqual({
			follow: false
		});
	});

	it('treats the page-boundary episode itself correctly', () => {
		// Ep 10 is the LAST tile of page 2 (ceil(10/5) = 2), not the
		// first of page 3 — an off-by-one here follows a page early.
		expect(decideStripFollow({ prevEpisode: 9, episode: 10, currentPage: 2, pageSize: 5 })).toEqual(
			{ follow: false }
		);
		expect(
			decideStripFollow({ prevEpisode: 10, episode: 11, currentPage: 2, pageSize: 5 })
		).toEqual({ follow: true, page: 3 });
	});

	it('refuses invalid inputs', () => {
		expect(
			decideStripFollow({ prevEpisode: 5, episode: NaN, currentPage: 1, pageSize: 5 })
		).toEqual({ follow: false });
		expect(decideStripFollow({ prevEpisode: 5, episode: 0, currentPage: 1, pageSize: 5 })).toEqual({
			follow: false
		});
		expect(decideStripFollow({ prevEpisode: 5, episode: 6, currentPage: 1, pageSize: 0 })).toEqual({
			follow: false
		});
	});
});
