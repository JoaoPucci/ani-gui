/**
 * What an episode tile's meta line says under its title, on a show's
 * page and in the player's episode strip.
 */

/** The meta line: an unaired episode's airing, an episode the
 *  streaming catalogue does not list yet, the episode's length, or
 *  nothing. */
export type TileMeta =
	| { kind: 'unaired' }
	| { kind: 'not-listed' }
	| { kind: 'length'; minutes: number }
	| null;

/** The tile's state, as the page knows it. */
export interface TileState {
	/** Not aired yet, by the schedule. */
	unaired: boolean;
	/** Whether the tile shows a picture (else a placeholder). */
	hasThumb: boolean;
	/** Past the catalogue's count, so asked about again on a click. */
	notListed: boolean;
	/** The episode's length in minutes, if known. */
	length: number | null;
}

export function tileMeta(state: TileState): TileMeta {
	void state;
	return null;
}
