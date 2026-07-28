/**
 * The Continue Watching click controller. Owns the whole resume
 * workflow — busy state, the shared settings await, the click-time
 * cap resolution, PiP session reuse, play resolution, the
 * watched/tracker fan-out, and error handling — so the home
 * component's handler is a thin state/navigation adapter and the
 * sequencing is unit-testable (AGENTS.md §3).
 *
 * Sequencing contract (pinned by start-resume.test.ts):
 *   guard busy/title → busy on → settings → episode resolution
 *   (probed cap as-is; otherwise ONE interactive lookup) → cap
 *   write-back → PiP reuse shortcut OR play resolution → watched +
 *   tracker fan-out (fire-and-forget) → navigate. Busy stays set on
 *   success — navigation unmounts the page; failure clears busy and
 *   reports through onFailure.
 */

import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import { resolveResumeEpisode } from './resume-episode';

export interface ResumePlayArgs {
	match: KitsuAnimeRef;
	title: string;
	episode: number;
	mode: 'sub' | 'dub';
	quality: string;
}

export interface StartResumeDeps {
	isBusy: () => boolean;
	onBusy: (kitsuId: string | null) => void;
	onProgress: (label: string | null) => void;
	onFailure: (title: string, error: unknown) => void;
	/** Awaits the page's shared settings load (resolveResumeSettings). */
	getSettings: () => Promise<{ mode: 'sub' | 'dub'; quality: string }>;
	/** Whether settings have actually landed — gates whether reuse
	 *  gets the resolved quality/mode or must match on (id, episode)
	 *  only, so a live PiP session at a non-default setting isn't
	 *  torn down on a config-less click. */
	settingsLoaded: () => boolean;
	getPlayableCount: (entryId: string) => number | null;
	setPlayableCount: (entryId: string, count: number) => void;
	/** Interactive (non-background) availability lookup — the gate
	 *  must neither pace nor breaker-refuse a user-awaited request. */
	fetchInteractiveCount: (match: KitsuAnimeRef, mode: 'sub' | 'dub') => Promise<number | null>;
	/** Persistent-PiP session reuse (reuseSessionIfMatching). */
	reuseSession: (
		kitsuId: string,
		episode: number,
		quality?: string,
		mode?: 'sub' | 'dub'
	) => { session_id: string; episode: number; media_kind: string } | null;
	/** getOrFire + playStream, reporting progress labels. */
	resolvePlay: (
		args: ResumePlayArgs,
		onProgress: (label: string) => void
	) => Promise<{ session_id: string }>;
	markWatched: (args: ResumePlayArgs) => Promise<void>;
	syncTrackers: (
		kitsuId: string,
		episode: number,
		seriesTotal: number | null,
		seriesFinished: boolean
	) => Promise<void>;
	navigateToCached: (
		kitsuId: string,
		cached: NonNullable<ReturnType<StartResumeDeps['reuseSession']>>
	) => void;
	navigateToSession: (
		kitsuId: string,
		session: { session_id: string },
		episode: number,
		quality: string,
		mode: 'sub' | 'dub'
	) => void;
}

export function makeStartResume(
	deps: StartResumeDeps
): (
	entry: HistoryEntry,
	match: KitsuAnimeRef,
	seriesTotal: number | null,
	seriesFinished: boolean
) => Promise<void> {
	return async (entry, match, seriesTotal, seriesFinished) => {
		if (deps.isBusy()) return;
		const title = match.canonical_title;
		if (!title) return;
		deps.onBusy(match.id);
		deps.onProgress(null);

		const { mode, quality } = await deps.getSettings();
		const lastWatchedRaw = parseInt(entry.ep_no, 10);
		const { episode, count } = await resolveResumeEpisode(
			Number.isFinite(lastWatchedRaw) ? lastWatchedRaw : null,
			deps.getPlayableCount(entry.id),
			match.episode_count ?? null,
			() => deps.fetchInteractiveCount(match, mode),
			// Re-read rather than reuse the snapshot above: the
			// background probe can publish an exact cap while the
			// interactive lookup is in flight, and that beats falling
			// back to Kitsu's optimistic count if the lookup fails.
			() => deps.getPlayableCount(entry.id)
		);
		if (typeof count === 'number') {
			deps.setPlayableCount(entry.id, count);
		}

		const loaded = deps.settingsLoaded();
		const cached = deps.reuseSession(
			match.id,
			episode,
			loaded ? quality : undefined,
			loaded ? mode : undefined
		);
		if (cached) {
			deps.navigateToCached(match.id, cached);
			return;
		}

		const args: ResumePlayArgs = { match, title, episode, mode, quality };
		try {
			const session = await deps.resolvePlay(args, (label) => deps.onProgress(label));
			void deps.markWatched(args).catch(() => {});
			void deps.syncTrackers(match.id, episode, seriesTotal, seriesFinished).catch(() => {});
			deps.navigateToSession(match.id, session, episode, quality, mode);
		} catch (e) {
			deps.onBusy(null);
			deps.onProgress(null);
			deps.onFailure(title, e);
		}
	};
}
