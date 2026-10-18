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
import { pickNextEpisode, resumeOr } from '$lib/play/next-episode';
import { parseHistoryEpisode } from '$lib/play/episode-number';

export interface ResumePlayArgs {
	/** Whether the card's match is only a guess (row-trust.ts): its id
	 *  is then not recorded, and the session is opened as a guess. */
	guess?: boolean;
	match: KitsuAnimeRef;
	title: string;
	episode: number;
	mode: 'sub' | 'dub';
	quality: string;
}

export interface StartResumeDeps {
	isBusy: () => boolean;
	/** Whether the clicked row's match is only a guess (row-trust.ts). */
	isGuess?: (entryId: string) => boolean;
	onBusy: (kitsuId: string | null) => void;
	onProgress: (label: string | null) => void;
	/** `args` is what the failed play asked for — the show, episode
	 *  and audio a failure surface needs to word its message. Busy
	 *  holds until a returned promise settles. */
	onFailure: (title: string, error: unknown, args: ResumePlayArgs) => void | Promise<void>;
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
	) => Promise<{ session_id: string; episode?: string; former_episode?: string }>;
	markWatched: (args: ResumePlayArgs) => Promise<void>;
	syncTrackers: (
		kitsuId: string,
		episode: number,
		seriesTotal: number | null,
		seriesFinished: boolean
	) => Promise<void>;
	navigateToSession: (
		kitsuId: string,
		session: { session_id: string },
		episode: number,
		guess?: boolean,
		row?: string
	) => void;
	/** Claims navigation as the click starts ($lib/navigation/claims).
	 *  A click whose claim lapsed while it worked — its page left, or a
	 *  newer navigation started — opens no player and shows no failure:
	 *  the viewer has moved on. Omitted, no claim lapses. */
	claimNavigation?: () => () => boolean;
	/** Whether `episode` of `kitsuId` was left part-way, its position
	 *  kept. Omitted, nothing was. */
	leftPartWay?: (kitsuId: string, episode: number) => boolean;
	/** Marks `episode` of `kitsuId` started — before the resolve, which
	 *  records the watch. Omitted, nothing is marked. A local position
	 *  keyed by the card's show, written for a guessed match too: it is
	 *  neither the history row nor a tracker, and removing the card
	 *  forgets it (delete-controller.ts), by `row`, the clicked history
	 *  row's id, whatever show the card names by then. */
	markStarted?: (kitsuId: string, episode: number, row?: string) => void;
	/** What the clicked history row reads as now, asked once the cap is
	 *  settled — the lookup that settles it can put the row's numbering
	 *  right. Omitted, the row reads as the card read it. */
	currentNumber?: (entryId: string) => Promise<{ ep_no: string; former_ep_no?: string } | null>;
	/** Keeps what was kept for `from` of `kitsuId` under `to` instead.
	 *  Omitted, nothing moves. */
	movePosition?: (kitsuId: string, from: number, to: number) => void;
	/** Resolves `args` without recording a watch — a prefetch — so the
	 *  backend fetches the show's listing. Omitted, nothing is warmed. */
	warm?: (args: ResumePlayArgs) => Promise<unknown>;
	/** The episode the opened session plays ({@link followPlayed}).
	 *  Omitted, the one asked for. */
	followPlayed?: (
		kitsuId: string,
		session: { episode?: string; former_episode?: string },
		requested: number
	) => number;
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
		const lastWatchedRaw = parseHistoryEpisode(entry.ep_no) ?? NaN;
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
		// The row as it reads now: the lookup that settled the cap pays
		// for the listing that puts its numbering right, and the card
		// read it before. A position kept under what it read as moves.
		const card = Number.isFinite(lastWatchedRaw) ? lastWatchedRaw : null;
		const guess = deps.isGuess?.(entry.id) ?? false;
		const now = await settledRow(deps, entry.id, card, count === null || approximate, {
			match,
			title,
			mode,
			quality,
			guess
		});
		if (now.former !== null && now.last !== null) {
			deps.movePosition?.(match.id, now.former, now.last);
		}
		const last = now.last;
		const next =
			last === card
				? resolved.episode
				: pickNextEpisode(last, count ?? match.episode_count ?? null);
		// The last watched episode was left part-way: going back to it,
		// not on to the next, is what Continue means.
		const episode = resumeOr(
			last,
			next,
			last !== null && (deps.leftPartWay?.(match.id, last) ?? false)
		);
		if (typeof count === 'number') {
			deps.setPlayableCount(entry.id, count, approximate);
		}

		const args: ResumePlayArgs = { match, title, episode, mode, quality, guess };
		deps.markStarted?.(match.id, episode, entry.id);
		try {
			const session = await deps.resolvePlay(args, (label) => deps.onProgress(label));
			// The episode the session plays, which the request may have
			// named by a number it no longer carries.
			const played = deps.followPlayed?.(match.id, session, episode) ?? episode;
			// Marked as the episode played: a resolve the request named by a
			// former tag is kept under it.
			void deps.markWatched({ ...args, episode: played }).catch(() => {});
			// A guessed match writes nothing to the user's tracker accounts.
			if (!guess) {
				void deps.syncTrackers(match.id, played, seriesTotal, seriesFinished).catch(() => {});
			}
			deps.navigateToSession(match.id, session, played, guess, entry.id);
		} catch (e) {
			deps.onProgress(null);
			// Busy holds until the failure is on screen: the page words it
			// after a cache lookup, and a card left idle meanwhile would
			// take a second click whose play the late message then covers.
			try {
				await deps.onFailure(title, e, args);
			} finally {
				deps.onBusy(null);
			}
		}
	};
}

/** What the clicked row reads as now, and the number it read as
 *  before its numbering was put right; what the card read when the row
 *  cannot be asked. */
async function rowNow(
	deps: StartResumeDeps,
	entryId: string,
	card: number | null
): Promise<{ last: number | null; former: number | null }> {
	try {
		const row = await deps.currentNumber?.(entryId);
		if (!row) return { last: card, former: null };
		return {
			last: parseHistoryEpisode(row.ep_no),
			former: parseHistoryEpisode(row.former_ep_no)
		};
	} catch {
		return { last: card, former: null };
	}
}

/** {@link rowNow}, with the row's own episode warmed first when no
 *  listing stands behind the cap (`unmeasured`) and the row reads as a
 *  fractional episode — the one kind a renumbering leaves under a
 *  number it no longer has. The warm resolves the listing, which puts
 *  the row right, and records no watch. */
async function settledRow(
	deps: StartResumeDeps,
	entryId: string,
	card: number | null,
	unmeasured: boolean,
	play: Omit<ResumePlayArgs, 'episode'>
): Promise<{ last: number | null; former: number | null }> {
	const now = await rowNow(deps, entryId, card);
	if (!unmeasured || !deps.warm || now.last === null || Number.isInteger(now.last)) return now;
	try {
		await deps.warm({ ...play, episode: now.last });
	} catch {
		return now;
	}
	return rowNow(deps, entryId, now.last);
}
