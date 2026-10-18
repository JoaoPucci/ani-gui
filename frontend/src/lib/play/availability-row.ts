/**
 * What the player knows about the provider's row for the show: whether
 * it lists the show, whether that is a refusal by name, the playable
 * count and the extra episodes it streams.
 *
 * The page writes every availability answer it accepts here — the
 * page-load lookup's and a cap-gate re-ask's — through `mergeRow`, and
 * reads the row back through `rowFor`. Each answer is stamped with the
 * show and audio it was asked for (`audioKey`).
 *
 * A patch writes only the fields it carries
 * ($lib/detail/availability-writeback): absent keys belong to someone
 * else, and the row keeps what it had for them. A lookup that fails or
 * comes back unsettled writes nothing, so the row keeps its verdict
 * then too — for the audio that verdict was about.
 *
 * An answer is about one show in one audio: the provider catalogues
 * sub and dub separately, and a show it dropped in one audio can be
 * listed in the other, at another count. So the row is read only for
 * the key its answers were for. Asked for any other — settings landing
 * with dub after the default sub question answered, while the dub one
 * is out, or after it failed or came back unsettled — it reads as
 * nothing known, which is how the page reads availability before any
 * answer: no play is refused for the show being delisted or for an
 * episode past a count, and the player-error overlay keeps its Reload
 * ($lib/play/reload-offer). The first answer about a new key starts
 * from nothing known as well, so a patch carrying only some fields
 * does not inherit the others from another audio.
 */

import type { AvailabilityPatch } from '$lib/detail/availability-writeback';

/** The show and audio an answer is about, or `null` before the page
 *  knows the show. */
export function audioKey(show: string | null | undefined, mode: 'sub' | 'dub'): string | null {
	return show ? `${show}:${mode}` : null;
}

/** The row as the page reads it. */
export interface RowView {
	/** Whether the provider lists the show; `null` until an answer. */
	listed: boolean | null;
	/** Whether `listed: false` is the provider refusing the title by
	 *  name. */
	refused: boolean;
	/** The playable count; `null` until an answer carries one. */
	count: number | null;
	/** Non-integer episode tags the provider streams (recaps). */
	extraEpisodes: readonly string[];
}

/** The row as stored: the view plus the key its answers were for. */
export interface AvailabilityRow extends RowView {
	key: string;
}

const NO_EXTRAS: readonly string[] = Object.freeze([]);

/** Nothing answered yet. */
export const UNKNOWN_ROW: RowView = Object.freeze({
	listed: null,
	refused: false,
	count: null,
	extraEpisodes: NO_EXTRAS
});

/** The row after `patch`, an answer about `key`. */
export function mergeRow(
	row: AvailabilityRow | null,
	key: string | null,
	patch: AvailabilityPatch
): AvailabilityRow | null {
	if (key === null) return row;
	const next: AvailabilityRow = { ...(row?.key === key ? row : UNKNOWN_ROW), key };
	if (patch.available !== undefined) {
		next.listed = patch.available;
		// Whether that verdict is a refusal by name travels with it.
		next.refused = patch.refused === true;
	}
	if (patch.count !== undefined) next.count = patch.count;
	if (patch.extraEpisodes !== undefined) next.extraEpisodes = patch.extraEpisodes;
	return next;
}

/** What the page knows about the row for `key`: nothing, unless its
 *  answers were about that show in that audio. */
export function rowFor(row: AvailabilityRow | null, key: string | null): RowView {
	return row !== null && row.key === key ? row : UNKNOWN_ROW;
}
