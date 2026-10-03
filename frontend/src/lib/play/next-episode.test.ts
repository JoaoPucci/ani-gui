import { describe, expect, it } from 'vitest';
import { pickNextEpisode, pickResumeEpisode } from './next-episode';

/** Episode the player should jump to when the user clicks Continue.
 *
 *  Mirrors the rules baked into the detail page's `defaultEpisode()`
 *  so the home Continue Watching card and the detail-page CTA agree
 *  on what "Continue" means. Centralising the rule means both
 *  surfaces handle replay-at-end + missing-cap + bad-history the
 *  same way without copy-pasting. */
describe('pickNextEpisode', () => {
	it('returns 1 when the user has no history (lastWatched is null)', () => {
		// Used by the detail page when there's no resume entry. Home
		// Continue Watching cards never hit this branch (they only
		// render for entries with history), but the helper has to
		// answer to both call sites with one signature.
		expect(pickNextEpisode(null, 12)).toBe(1);
		expect(pickNextEpisode(null, null)).toBe(1);
	});

	it('returns 1 for malformed lastWatched (NaN / non-finite)', () => {
		// Defensive: history rows are user-editable text;
		// parseInt-then-pass should always land on a sane default.
		expect(pickNextEpisode(Number.NaN, 12)).toBe(1);
		expect(pickNextEpisode(Number.POSITIVE_INFINITY, 12)).toBe(1);
	});

	it('returns 1 when lastWatched is below 1 (defensive)', () => {
		expect(pickNextEpisode(0, 12)).toBe(1);
		expect(pickNextEpisode(-3, 12)).toBe(1);
	});

	it('returns lastWatched + 1 mid-show', () => {
		expect(pickNextEpisode(5, 12)).toBe(6);
		expect(pickNextEpisode(1, 24)).toBe(2);
	});

	it('returns lastWatched (replay) when the next would exceed the cap', () => {
		// Detail page's "Replay · Episode N" branch. Home card hits
		// the same path for a movie (lastWatched=1, cap=1 → 2 > 1,
		// fall back to 1) — so single-video shows resolve to "play
		// the only episode again" without a separate isSingleVideo
		// branch.
		expect(pickNextEpisode(12, 12)).toBe(12);
		expect(pickNextEpisode(1, 1)).toBe(1); // movie / 1-ep finished
	});

	it('returns lastWatched + 1 when the cap is unknown', () => {
		// Kitsu sometimes omits episode_count (especially for ONA /
		// upcoming). Without a cap we can't guard against overshoot
		// — but the backend's availability filter on the surrounding
		// surfaces (home rows, dropdown) already drops shows that
		// aren't streamable, so a phantom "episode 100" click would
		// land on a failure the lazy click path surfaces normally.
		// Match the detail page's existing behaviour.
		expect(pickNextEpisode(5, null)).toBe(6);
	});
});

describe('pickResumeEpisode', () => {
	// The episode a Continue click plays. Leaving an episode part-way
	// keeps its position, and coming back means going back to it — not
	// to the next one, which is where the watched mark, written when
	// the episode started, would otherwise send the viewer.
	it('returns the last watched episode when it was left part-way', () => {
		expect(pickResumeEpisode(5, 12, true)).toBe(5);
	});

	it('returns the next episode when the last one was not left part-way', () => {
		expect(pickResumeEpisode(5, 12, false)).toBe(6);
	});

	it("returns a series' last episode, left part-way, as itself", () => {
		expect(pickResumeEpisode(12, 12, true)).toBe(12);
	});

	it('returns episode 1 without history, whatever the flag says', () => {
		expect(pickResumeEpisode(null, 12, true)).toBe(1);
		expect(pickResumeEpisode(Number.NaN, 12, true)).toBe(1);
		expect(pickResumeEpisode(0, 12, true)).toBe(1);
	});
});
