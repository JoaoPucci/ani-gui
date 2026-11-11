/**
 * The pick a player mounted again finds its line keeps.
 *
 * On the player, a pick of another show answered with "Keep watching
 * this" leaves the episode playing while the line records the pick
 * (`named`): the session's later episodes go out as the pick
 * ($lib/play/session-show.ts), and the slip says so with the way to
 * switch. That lives in the player's own state, and the URL still names
 * the show playing. A player mounted again (a reload, a recovery) reads
 * it back from the line: a line the user named for another show than the
 * one playing is that pick, kept.
 */

import type { KitsuAnimeRef, ReadOptions } from '$lib/api';
import type { LinkOffer } from './link-offer';
import type { LinkDeps } from './link-offer';
import { readOpts } from './read-opts';

/** The show the line `row` was named for, when it is not `pageId`, the
 *  show playing; null otherwise, and when it cannot be read. */
export async function keptPick(
	deps: LinkDeps,
	row: string,
	pageId: string,
	opts?: ReadOptions
): Promise<KitsuAnimeRef | null> {
	try {
		const entry = (await deps.history(...readOpts(opts ?? {}))).find((e) => e.id === row);
		if (!entry?.named) return null;
		const { match } = await deps.resolve(entry, ...readOpts(opts ?? {}));
		return match && match.id !== pageId ? match : null;
	} catch {
		return null;
	}
}

/** The pick an arrival on the player finds kept, for the arrival that
 *  read it: only where its offer was read and none was made. */
export async function keptOnArrival(
	deps: LinkDeps,
	at: { inPlayer: boolean; row: string | null; pageId: string },
	found: { offer: LinkOffer | null; unread: boolean },
	opts?: ReadOptions
): Promise<KitsuAnimeRef | null> {
	if (!at.inPlayer || !at.row || found.offer !== null || found.unread) return null;
	return keptPick(deps, at.row, at.pageId, opts);
}

/** The picks arrivals found kept, each taken only by its own arrival:
 *  an earlier one's read landing late is not the later one's. */
export class KeptPicks {
	#held: { arrival: number; pick: KitsuAnimeRef } | null = null;

	note(arrival: number, pick: KitsuAnimeRef | null): void {
		this.#held = pick ? { arrival, pick } : null;
	}

	/** The pick arrival `arrival` found, as the session names a link. */
	take(arrival: number): { id: string; title: string; show: KitsuAnimeRef } | null {
		const held = this.#held;
		this.#held = null;
		if (!held || held.arrival !== arrival) return null;
		return { id: held.pick.id, title: held.pick.canonical_title, show: held.pick };
	}
}
