/**
 * The Continue Watching click controller. Owns the whole resume
 * workflow — busy state, the shared settings await, the click-time
 * cap resolution, play resolution, the watched/tracker fan-out, and
 * error handling — so the home
 * component's handler is a thin state/navigation adapter and the
 * sequencing is unit-testable (AGENTS.md §3).
 *
 * Sequencing contract (pinned by start-resume.test.ts):
 *   guard busy/title → busy on → settings → episode resolution
 *   (probed cap as-is; otherwise ONE interactive lookup; the last
 *   watched episode again when it was left part-way) → cap
 *   write-back → play resolution → watched + tracker fan-out
 *   (fire-and-forget) → navigate. Busy stays set on
 *   success — navigation unmounts the page; failure clears busy and
 *   reports through onFailure.
 */

import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import { resolveResumeEpisode } from './resume-episode';
import { resumeOr } from '$lib/play/next-episode';

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
	getPlayableCount: (entryId: string) => number | null;
	/** Whether the stored cap came from the search hit rather than the
	 *  detail fetch. An approximate cap is revalidated before use —
	 *  provenance decides, not the episode's position, because the
	 *  overcount is not bounded to one. Optional so callers without
	 *  provenance keep the previous behaviour. */
	isPlayableCountApproximate?: (entryId: string) => boolean;
	setPlayableCount: (entryId: string, count: number, approximate: boolean) => void;
	/** Interactive (non-background) availability lookup — the gate
	 *  must neither pace nor breaker-refuse a user-awaited request. */
	/** Interactive cap lookup. Returns the count AND whether it came
	 *  from the search hit rather than the detail fetch — the
	 *  interactive lane bypasses the gate, but its detail fetch can
	 *  still fail, and collapsing that to a bare number would let an
	 *  approximate answer be recorded as confirmed. */
	fetchInteractiveCount: (
		match: KitsuAnimeRef,
		mode: 'sub' | 'dub'
	) => Promise<{ count: number | null; approximate: boolean }>;
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
	navigateToSession: (kitsuId: string, session: { session_id: string }, episode: number) => void;
	/** Whether `episode` of `kitsuId` was left part-way, its position
	 *  kept. Omitted, nothing was. */
	leftPartWay?: (kitsuId: string, episode: number) => boolean;
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
		const resolved = await resolveResumeEpisode(
			Number.isFinite(lastWatchedRaw) ? lastWatchedRaw : null,
			deps.getPlayableCount(entry.id),
			match.episode_count ?? null,
			() => deps.fetchInteractiveCount(match, mode),
			// Re-read rather than reuse the snapshot above: the
			// background probe can publish an exact cap while the
			// interactive lookup is in flight, and that beats falling
			// back to Kitsu's optimistic count if the lookup fails.
			() => ({
				count: deps.getPlayableCount(entry.id),
				approximate: deps.isPlayableCountApproximate?.(entry.id) ?? false
			}),
			deps.isPlayableCountApproximate?.(entry.id) ?? false
		);
		const { count, approximate } = resolved;
		// The last watched episode was left part-way: going back to it,
		// not on to the next, is what Continue means.
		const last = Number.isFinite(lastWatchedRaw) ? lastWatchedRaw : null;
		const episode = resumeOr(
			last,
			resolved.episode,
			last !== null && (deps.leftPartWay?.(match.id, last) ?? false)
		);
		if (typeof count === 'number') {
			deps.setPlayableCount(entry.id, count, approximate);
		}

		const args: ResumePlayArgs = { match, title, episode, mode, quality };
		try {
			const session = await deps.resolvePlay(args, (label) => deps.onProgress(label));
			void deps.markWatched(args).catch(() => {});
			void deps.syncTrackers(match.id, episode, seriesTotal, seriesFinished).catch(() => {});
			deps.navigateToSession(match.id, session, episode);
		} catch (e) {
			deps.onBusy(null);
			deps.onProgress(null);
			deps.onFailure(title, e);
		}
	};
}
