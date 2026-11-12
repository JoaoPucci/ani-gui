/**
 * The pick a player mounted again finds its line keeps.
 *
 * On the player, a pick of another show answered with "Keep watching
 * this" leaves the episode playing while the line records the pick
 * (`named`): the session's later episodes go out as the pick
 * ($lib/play/session-show.ts), and the slip says so with the way to
 * switch. That lives in the player's own state, and the URL still names
 * the show playing. A player mounted again — the renderer reloaded, or
 * Back to the player a switch left — reads it back from the line: a line
 * the user named for another show than the one playing is that pick,
 * kept, as the backend's verdict says (`commands/link_verdict.rs`). It
 * brings back the pick and the slip's way to switch, and the verdict
 * the ⋯ entry to change it again, on the show playing.
 */

import type { KitsuAnimeRef } from '$lib/api';
import type { SurfaceFind } from './picked-offer';

/** The pick an arrival on the player finds kept, for the arrival that
 *  read it: the show its line was named for when another plays
 *  (`SurfaceFind.kept`, the backend's verdict), beside the offer to
 *  change it again; none where the line could not be read. */
export function keptOnArrival(
	at: { inPlayer: boolean; row: string | null },
	found: SurfaceFind
): KitsuAnimeRef | null {
	if (!at.inPlayer || !at.row || found.unread) return null;
	return found.kept;
}

/** The picks arrivals found kept, each taken only by its own arrival:
 *  an earlier one's read landing late is not the later one's. */
export class KeptPicks {
	#held: { arrival: number; pick: KitsuAnimeRef } | null = null;
	#latest = 0;

	note(arrival: number, pick: KitsuAnimeRef | null): void {
		if (arrival < this.#latest) return;
		this.#latest = arrival;
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
