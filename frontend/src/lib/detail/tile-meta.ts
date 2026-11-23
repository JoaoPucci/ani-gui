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

/**
 * The meta line for a tile. An unaired episode's airing shows here
 * under a picture; without one, its placeholder shows it. An episode
 * past the catalogue is said to be not listed yet, in place of its
 * length: dimmed colours alone, which a contrast theme replaces, would
 * not say it. Otherwise the length, when known.
 */
export function tileMeta({ unaired, hasThumb, notListed, length }: TileState): TileMeta {
	if (unaired) return hasThumb ? { kind: 'unaired' } : null;
	if (notListed) return { kind: 'not-listed' };
	return length ? { kind: 'length', minutes: length } : null;
}
