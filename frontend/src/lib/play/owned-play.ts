/**
 * Who a play belongs to, and what its answer may still do.
 *
 * SvelteKit reuses a route's component when the viewer follows a link
 * to another show, and when they go away and come back. A play the
 * viewer started can therefore answer on a visit they have left. The
 * pages keep a visit counter that advances on every change of the
 * show id; a play claims the show and the visit it was started for,
 * and its answer is judged against that claim:
 *
 *   • a success on the same visit is played and current — the page
 *     records the watch and opens the player;
 *   • a success after a move is played but not current — the watch is
 *     still the clicked show's, but no player opens;
 *   • a failure on the same visit is worded (which may wait on a cache
 *     lookup) and shown, if the visit is still the same once worded;
 *   • a failure after a move, or one the viewer moved away from while
 *     it was being worded, is abandoned: nothing is shown, and nothing
 *     is asked of the cache for a visit nobody is on.
 *
 * The pages are adapters: they claim, run, and act on the outcome.
 */

import { altTitlesFromKitsu, yearFromKitsuRef, type KitsuAnimeRef, type PlayArgs } from '$lib/api';

/** A play's claim on the show and the route visit it was started for. */
export interface PlayOwner {
	readonly showId: string;
	readonly visit: number;
	/** Whether the route is still on the visit the play was started on. */
	isCurrent(): boolean;
}

/** Claim a play for `showId` on the route's current visit. `visit`
 *  reads the route's visit counter; it is read now and on every
 *  `isCurrent()`. `gone` reads whether the page was destroyed, which
 *  leaving the route does without moving the counter. */
export function claimPlay(
	showId: string,
	visit: () => number,
	gone: () => boolean = () => false
): PlayOwner {
	const claimed = visit();
	return { showId, visit: claimed, isCurrent: () => !gone() && visit() === claimed };
}

/** What a play's answer may still do (see the module notes). */
export type PlayOutcome<S> =
	| { kind: 'played'; session: S; current: boolean }
	| { kind: 'failed'; error: unknown; message: string }
	| { kind: 'abandoned' };

/** Run `attempt` for `owner` and judge its answer. `describeFailure`
 *  words a failure; it runs only while the play is still current, and
 *  a move while it runs abandons the failure. */
export async function runOwnedPlay<S>(
	owner: PlayOwner,
	attempt: () => Promise<S>,
	describeFailure: (e: unknown) => Promise<string>
): Promise<PlayOutcome<S>> {
	let session: S;
	try {
		session = await attempt();
	} catch (e) {
		if (!owner.isCurrent()) return { kind: 'abandoned' };
		const message = await describeFailure(e);
		if (!owner.isCurrent()) return { kind: 'abandoned' };
		return { kind: 'failed', error: e, message };
	}
	return { kind: 'played', session, current: owner.isCurrent() };
}

/** The play request for `episode` of `show`, built once at the click
 *  so a late answer records the show the viewer clicked. `kitsuId` is
 *  left off when the play records none (a guessed match). */
export function playArgsFor(input: {
	title: string;
	episode: number;
	mode: 'sub' | 'dub';
	quality: string;
	show: KitsuAnimeRef | null;
	kitsuId: string | undefined;
}): PlayArgs {
	const args: PlayArgs = {
		title: input.title,
		episode: String(input.episode),
		mode: input.mode,
		quality: input.quality,
		episode_count: input.show?.episode_count ?? null,
		year: yearFromKitsuRef(input.show),
		subtype: input.show?.subtype ?? null,
		alt_titles: altTitlesFromKitsu(input.show)
	};
	if (input.kitsuId !== undefined) args.kitsu_id = input.kitsuId;
	return args;
}
