import { describe, expect, it } from 'vitest';
import { StripPager } from './strip-pager';

/** A pager that has settled on the page holding `episode`, the state
 *  the play page is in right after its mount fetch lands. */
function settledAt(episode: number, pageSize = 5): StripPager {
	const pager = new StripPager(pageSize);
	const req = pager.open(pager.pageOf(episode));
	if (req.fetch) pager.completed(req.gen);
	return pager;
}

describe('StripPager', () => {
	describe('pageOf', () => {
		it('maps episodes to 1-based pages, boundary tiles included', () => {
			const pager = new StripPager(5);
			expect(pager.pageOf(1)).toBe(1);
			expect(pager.pageOf(5)).toBe(1);
			expect(pager.pageOf(6)).toBe(2);
			// Ep 10 is the LAST tile of page 2, not the first of page 3 —
			// an off-by-one here follows a page early.
			expect(pager.pageOf(10)).toBe(2);
			expect(pager.pageOf(11)).toBe(3);
		});

		it('falls back to page 1 on invalid input', () => {
			expect(new StripPager(5).pageOf(NaN)).toBe(1);
			expect(new StripPager(5).pageOf(0)).toBe(1);
			expect(new StripPager(0).pageOf(6)).toBe(1);
		});
	});

	describe('following playback', () => {
		it('follows a next-press off the end of the visible page', () => {
			// The originally reported bug: watching ep 5 with the strip
			// on page 1, Next lands on ep 6 and the strip must fetch
			// page 2.
			const pager = settledAt(5);
			expect(pager.episodeChanged(6)).toEqual({ fetch: true, page: 2, gen: 2 });
		});

		it('follows a prev-press back across the page boundary', () => {
			const pager = settledAt(6);
			expect(pager.episodeChanged(5)).toEqual({ fetch: true, page: 1, gen: 2 });
		});

		it('follows a multi-page jump while tracking', () => {
			const pager = settledAt(5);
			expect(pager.episodeChanged(23)).toEqual({ fetch: true, page: 5, gen: 2 });
		});

		it('stays put when the new episode is already on the visible page', () => {
			const pager = settledAt(3);
			expect(pager.episodeChanged(4)).toEqual({ fetch: false });
		});

		it('does not issue a duplicate fetch while the same follow is in flight', () => {
			const pager = settledAt(5);
			expect(pager.episodeChanged(6)).toEqual({ fetch: true, page: 2, gen: 2 });
			// A same-episode re-landing (stale-stream recovery replaces
			// the URL) arrives before the fetch resolves: page 2 is
			// already on its way.
			expect(pager.episodeChanged(6)).toEqual({ fetch: false });
		});
	});

	describe('browsing', () => {
		it('stops following when the user paginates away', () => {
			// Ep 12 plays from page 3; the user browses to page 7.
			// Auto-play advancing 12 → 13 must not yank the strip —
			// carousels respond to the user, not to timers.
			const pager = settledAt(12);
			const browse = pager.userGoto(7, 12);
			expect(browse).toEqual({ fetch: true, page: 7, gen: 2 });
			if (browse.fetch) pager.completed(browse.gen);
			expect(pager.episodeChanged(13)).toEqual({ fetch: false });
		});

		it('re-latches when playback enters the browsed page', () => {
			// Ep 5 plays from page 1; the user browses to page 2 and the
			// episode then advances onto it. From there the strip is
			// tracking again: the next boundary cross follows.
			const pager = settledAt(5);
			const browse = pager.userGoto(2, 5);
			expect(browse).toEqual({ fetch: true, page: 2, gen: 2 });
			if (browse.fetch) pager.completed(browse.gen);
			expect(pager.episodeChanged(6)).toEqual({ fetch: false });
			expect(pager.episodeChanged(11)).toEqual({ fetch: true, page: 3, gen: 3 });
		});

		it('keeps tracking when the user paginates to the playing page', () => {
			// Browsing page 7, then paging back to the current episode's
			// page: that is a return to following, not more browsing.
			const pager = settledAt(12);
			const away = pager.userGoto(7, 12);
			if (away.fetch) pager.completed(away.gen);
			const back = pager.userGoto(3, 12);
			expect(back).toEqual({ fetch: true, page: 3, gen: 3 });
			if (back.fetch) pager.completed(back.gen);
			expect(pager.episodeChanged(16)).toEqual({ fetch: true, page: 4, gen: 4 });
		});

		it('declines a userGoto to the page already requested', () => {
			const pager = settledAt(5);
			expect(pager.userGoto(1, 5)).toEqual({ fetch: false });
		});
	});

	describe('racing fetches', () => {
		it('discards a stale mount-time completion after a follow superseded it', () => {
			const pager = new StripPager(5);
			const mount = pager.open(1);
			expect(mount).toEqual({ fetch: true, page: 1, gen: 1 });
			const follow = pager.episodeChanged(6);
			expect(follow).toEqual({ fetch: true, page: 2, gen: 2 });
			if (follow.fetch) expect(pager.completed(follow.gen)).toBe(true);
			if (mount.fetch) expect(pager.completed(mount.gen)).toBe(false);
		});

		it('supersedes an in-flight follow when navigation reverses onto the rendered page', () => {
			// 20 → 21 starts fetching page 5; Prev back to 20 before it
			// resolves. The rendered page never changed, but the
			// REQUESTED page did — the reversal must arm a superseding
			// fetch so the stale page-5 response loses the generation
			// race.
			const pager = settledAt(20);
			const follow = pager.episodeChanged(21);
			expect(follow).toEqual({ fetch: true, page: 5, gen: 2 });
			const reverse = pager.episodeChanged(20);
			expect(reverse).toEqual({ fetch: true, page: 4, gen: 3 });
			if (reverse.fetch) expect(pager.completed(reverse.gen)).toBe(true);
			if (follow.fetch) expect(pager.completed(follow.gen)).toBe(false);
		});

		it('keys the loading flag to the newest generation only', () => {
			const pager = new StripPager(5);
			const mount = pager.open(1);
			const follow = pager.episodeChanged(6);
			if (mount.fetch) expect(pager.isCurrent(mount.gen)).toBe(false);
			if (follow.fetch) expect(pager.isCurrent(follow.gen)).toBe(true);
		});
	});

	describe('failed fetches', () => {
		it('surfaces only the newest generation failure', () => {
			const pager = new StripPager(5);
			const mount = pager.open(1);
			const follow = pager.episodeChanged(6);
			if (mount.fetch) expect(pager.failed(mount.gen)).toBe(false);
			if (follow.fetch) expect(pager.completed(follow.gen)).toBe(true);
		});

		it('retries the follow on the next episode change after a transient failure', () => {
			// A follow that failed was still TRACKING playback; the
			// failure must not reclassify the strip as browsed-away, or
			// every later episode change would decline the retry and the
			// strip would strand until manual pagination.
			const pager = settledAt(20);
			const follow = pager.episodeChanged(21);
			expect(follow).toEqual({ fetch: true, page: 5, gen: 2 });
			if (follow.fetch) expect(pager.failed(follow.gen)).toBe(true);
			expect(pager.episodeChanged(22)).toEqual({ fetch: true, page: 5, gen: 3 });
		});

		it('lets the user re-attempt the failed page immediately', () => {
			// Failure reverts the requested page to the rendered one, so
			// the already-there guard cannot swallow a manual retry
			// toward the page that just failed.
			const pager = settledAt(20);
			const browse = pager.userGoto(5, 20);
			expect(browse).toEqual({ fetch: true, page: 5, gen: 2 });
			if (browse.fetch) expect(pager.failed(browse.gen)).toBe(true);
			expect(pager.userGoto(5, 20)).toEqual({ fetch: true, page: 5, gen: 3 });
		});
	});

	describe('before the strip has opened', () => {
		it('decides against the default rendered page', () => {
			// No fetch has been issued yet, so decisions fall back to
			// the initial rendered page (1): an episode on it re-latches
			// without a fetch, one past it follows.
			expect(new StripPager(5).episodeChanged(3)).toEqual({ fetch: false });
			expect(new StripPager(5).episodeChanged(6)).toEqual({ fetch: true, page: 2, gen: 1 });
			expect(new StripPager(5).userGoto(1, 3)).toEqual({ fetch: false });
		});
	});

	describe('outcomes with no fetch issued', () => {
		it('reports nothing to apply and nothing to surface', () => {
			// gen starts at 0, so a stray completed(0)/failed(0) matches
			// the "newest generation" check on a pager that never issued
			// a fetch. There is no request whose data could be applied
			// or whose error could be surfaced — both must decline.
			expect(new StripPager(5).completed(0)).toBe(false);
			expect(new StripPager(5).failed(0)).toBe(false);
		});
	});

	describe('invalid inputs', () => {
		it('declines invalid episodes and pages', () => {
			const pager = settledAt(5);
			expect(pager.episodeChanged(NaN)).toEqual({ fetch: false });
			expect(pager.episodeChanged(0)).toEqual({ fetch: false });
			expect(pager.userGoto(0, 5)).toEqual({ fetch: false });
			expect(new StripPager(5).open(NaN)).toEqual({ fetch: false });
		});
	});
});
