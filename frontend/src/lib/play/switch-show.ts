/**
 * "Switch to it now": the player moves to the show the viewer picked
 * for its card, at the same episode and from the
 * same point, on that show's own stream.
 *
 * The pick is the user's, so the play asks for the picked show by its
 * own title and Kitsu id, with no guess beside it, as a play from its
 * page would; its watch is recorded and mirrored as one. Once the
 * stream is had, the point playback stood at is carried over to it
 * (the recovery's carry-over, `$lib/play/resume-after-recovery`), the
 * session it opens is told once that it plays the pick now
 * (`$lib/history/guess-slip`), and the player opens on it. The play
 * route mounts its page afresh for the other show
 * (`routes/play/[id]/+layout.svelte`).
 *
 * The episode is the one the session says it plays, which a renumbered
 * recap may name by another number than the one asked for, and the
 * playback speed goes along ($lib/play/rate-carry). The watch is
 * recorded once the player opened on the stream, and only then: the
 * resolve asks the backend to write no history (`record_later`), at a
 * click's priority. A stream that cannot
 * be had, or a player that cannot be opened on it, is a failed switch
 * that records nothing and leaves nothing for a later page: the
 * episode on screen goes on, and the caller says so. A viewer who left the player while
 * the stream was on its way is not taken back to it.
 *
 * The page is an adapter: it hands in the app's effects and shows the
 * between-episodes loading band while this runs.
 */

import type { CreateSessionResponse, KitsuAnimeRef, PlayArgs, VersionKind } from '$lib/api';
import { playArgsFor } from './owned-play';

export interface SwitchShowDeps {
	/** The stream for `args`. Rejects when there is none. */
	resolve: (args: PlayArgs) => Promise<CreateSessionResponse>;
	/** Record the watch the play stands for, of `episode`, and mirror
	 *  it. */
	record: (args: PlayArgs, session: CreateSessionResponse, episode: number) => void;
	/** Carry `at` seconds over to the next attach of `episode` of
	 *  `showId`. */
	carry: (showId: string, episode: number, at: number) => void;
	/** Tell the session of the card row `row` that it plays `title`. */
	noteSwitch: (row: string, title: string) => void;
	/** Open the player on `session`, the episode `episode` of `showId`,
	 *  from the card row `row`; whether it opened. */
	open: (
		showId: string,
		session: CreateSessionResponse,
		episode: number,
		row: string | null
	) => Promise<boolean>;
	/** The episode `session` plays, asked for as `requested`: the number
	 *  it has now, with the positions kept under the old one moved to it
	 *  (`followPlayed`, $lib/play/played-episode). */
	played: (showId: string, session: CreateSessionResponse, requested: number) => number;
	/** Hand the playback speed to the next player of `showId`
	 *  ($lib/play/rate-carry). */
	carryRate: (showId: string, rate: number) => void;
	/** Forget what a switch to `showId` from the card row `row` left for
	 *  the next page: the point, the speed and the note. */
	forget: (showId: string, row: string | null) => void;
	/** Whether the viewer is still on the player the switch began on. */
	isCurrent: () => boolean;
}

export interface SwitchRequest {
	/** The show picked for the card. */
	to: KitsuAnimeRef;
	/** The episode playing. */
	episode: number;
	/** Where playback stood, in seconds. */
	at: number;
	/** The card's history row, when a card opened the session. */
	row: string | null;
	mode: 'sub' | 'dub';
	quality: string;
	/** The version and captions remembered for the picked show. */
	version?: VersionKind;
	caption?: string;
	/** The playback speed, kept for the next player. */
	rate?: number;
}

export type SwitchOutcome = 'switched' | 'failed' | 'abandoned';

export async function switchShow(deps: SwitchShowDeps, req: SwitchRequest): Promise<SwitchOutcome> {
	const title = req.to.canonical_title;
	if (!title) return 'failed';
	const args = playArgsFor({
		title,
		episode: req.episode,
		mode: req.mode,
		quality: req.quality,
		show: req.to,
		kitsuId: req.to.id,
		version: req.version,
		caption: req.caption
	});
	let session: CreateSessionResponse;
	try {
		// The backend writes no history as it resolves: the watch is
		// recorded below, once the player opened on the stream.
		session = await deps.resolve({ ...args, record_later: true });
	} catch {
		return 'failed';
	}
	if (!deps.isCurrent()) return 'abandoned';
	// The session may name the episode by the number it has now.
	const episode = deps.played(req.to.id, session, req.episode);
	// What the next page takes as it mounts goes first.
	deps.carry(req.to.id, episode, req.at);
	if (req.rate !== undefined) deps.carryRate(req.to.id, req.rate);
	if (req.row) deps.noteSwitch(req.row, title);
	if (!(await deps.open(req.to.id, session, episode, req.row))) {
		deps.forget(req.to.id, req.row);
		return 'failed';
	}
	deps.record({ ...args, episode: String(episode) }, session, episode);
	return 'switched';
}
