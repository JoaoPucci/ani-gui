/**
 * Pure logic for the detail-page primary-action CTA label.
 *
 * isSingleVideo: detects "one-video" shows that should never read
 * "Play episode 1" — movies (always) and OVA/special/ONA whose
 * episode_count is 1 AND whose status is not "current" (excludes
 * ongoing TV that briefly reports a single aired episode).
 *
 * computePlayLabel: given the single-video flag, an optional resume
 * entry from ani-cli history, and the defaultEpisode the play
 * button would dispatch to, returns a discriminated state the
 * Svelte derived value maps to one of five i18n message keys.
 */
import { describe, it, expect } from 'vitest';
import { isSingleVideo, computePlayLabel } from './play-label';

describe('isSingleVideo', () => {
	it('returns true for movies regardless of episode_count or status', () => {
		expect(isSingleVideo('movie', null, null)).toBe(true);
		expect(isSingleVideo('movie', 1, 'finished')).toBe(true);
		expect(isSingleVideo('movie', null, 'upcoming')).toBe(true);
		// A "currently airing" movie is a Kitsu oddity, but it is still
		// one video by definition.
		expect(isSingleVideo('movie', 1, 'current')).toBe(true);
	});

	it('returns true for OVA/special/ONA with episode_count==1 and a non-current status', () => {
		expect(isSingleVideo('OVA', 1, 'finished')).toBe(true);
		expect(isSingleVideo('special', 1, 'finished')).toBe(true);
		expect(isSingleVideo('ONA', 1, 'finished')).toBe(true);
		expect(isSingleVideo('OVA', 1, 'upcoming')).toBe(true);
		expect(isSingleVideo('special', 1, 'tba')).toBe(true);
		expect(isSingleVideo('ONA', 1, 'unreleased')).toBe(true);
	});

	it('returns false for an ongoing TV show that currently has one aired episode', () => {
		// The whole point of gating on status — without this guard,
		// a newly-premiering TV series would mis-label as "Watch".
		expect(isSingleVideo('TV', 1, 'current')).toBe(false);
	});

	it('returns false for OVA/special/ONA marked currently airing with one episode', () => {
		// Same false-positive guard, broader subtype net.
		expect(isSingleVideo('OVA', 1, 'current')).toBe(false);
		expect(isSingleVideo('special', 1, 'current')).toBe(false);
	});

	it('returns false for multi-episode shows', () => {
		expect(isSingleVideo('TV', 12, 'finished')).toBe(false);
		expect(isSingleVideo('OVA', 4, 'finished')).toBe(false);
		expect(isSingleVideo('special', 3, 'finished')).toBe(false);
	});

	it('returns false when episode_count is unknown and subtype is not movie', () => {
		// Ongoing shows often report episode_count as null. Only the
		// movie shortcut overrides that.
		expect(isSingleVideo('TV', null, 'current')).toBe(false);
		expect(isSingleVideo('OVA', null, 'current')).toBe(false);
		expect(isSingleVideo('special', null, 'finished')).toBe(false);
	});

	it('returns false when both subtype and episode_count are absent', () => {
		expect(isSingleVideo(null, null, null)).toBe(false);
		expect(isSingleVideo(undefined, undefined, undefined)).toBe(false);
	});
});

describe('computePlayLabel', () => {
	it('returns watch when single-video and no resume entry', () => {
		expect(
			computePlayLabel({ isSingleVideo: true, resumeEntry: null, defaultEpisode: 1 })
		).toEqual({ kind: 'watch' });
	});

	it('returns watch_again when single-video and a resume entry exists', () => {
		expect(
			computePlayLabel({
				isSingleVideo: true,
				resumeEntry: { ep_no: '1' },
				defaultEpisode: 1
			})
		).toEqual({ kind: 'watch_again' });
	});

	it('returns episode_one when multi-episode and no resume entry', () => {
		expect(
			computePlayLabel({ isSingleVideo: false, resumeEntry: null, defaultEpisode: 1 })
		).toEqual({ kind: 'episode_one' });
	});

	it('returns resume with defaultEpisode when multi-episode and continuing past the last watched ep', () => {
		expect(
			computePlayLabel({
				isSingleVideo: false,
				resumeEntry: { ep_no: '5' },
				defaultEpisode: 6
			})
		).toEqual({ kind: 'resume', episode: 6 });
	});

	it('returns replay when multi-episode and defaultEpisode equals the last watched ep (capped at episode_count)', () => {
		// User watched ep 12, the cap is 12 so defaultEpisode saturates at last.
		expect(
			computePlayLabel({
				isSingleVideo: false,
				resumeEntry: { ep_no: '12' },
				defaultEpisode: 12
			})
		).toEqual({ kind: 'replay', episode: 12 });
	});

	it('treats single-video as watch_again even when defaultEpisode would have triggered replay otherwise', () => {
		// Movie with resume entry: defaultEpisode saturates at 1 (cap=1),
		// last=1, so the multi-ep code path would say "replay". Single-
		// video override wins.
		expect(
			computePlayLabel({
				isSingleVideo: true,
				resumeEntry: { ep_no: '1' },
				defaultEpisode: 1
			})
		).toEqual({ kind: 'watch_again' });
	});
});
