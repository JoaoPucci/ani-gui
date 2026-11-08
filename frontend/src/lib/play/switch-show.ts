/**
 * "Switch to it now": the player of a guessed session moves to the
 * show the viewer picked for its card, at the same episode and from the
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
 * A stream that cannot be had changes nothing: the episode on screen
 * goes on, and the caller says so. A viewer who left the player while
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
	/** Record the watch the play stands for, and mirror it. */
	record: (args: PlayArgs, session: CreateSessionResponse) => void;
	/** Carry `at` seconds over to the next attach of `episode` of
	 *  `showId`. */
	carry: (showId: string, episode: number, at: number) => void;
	/** Tell the session of the card row `row` that it plays `title`. */
	noteSwitch: (row: string, title: string) => void;
	/** Open the player on `session`, the episode `episode` of `showId`,
	 *  from the card row `row`. */
	open: (
		showId: string,
		session: CreateSessionResponse,
		episode: number,
		row: string | null
	) => Promise<void>;
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
		session = await deps.resolve(args);
	} catch {
		return 'failed';
	}
	if (!deps.isCurrent()) return 'abandoned';
	deps.record(args, session);
	deps.carry(req.to.id, req.episode, req.at);
	if (req.row) deps.noteSwitch(req.row, title);
	await deps.open(req.to.id, session, req.episode, req.row);
	return 'switched';
}
