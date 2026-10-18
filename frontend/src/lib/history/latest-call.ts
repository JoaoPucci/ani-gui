/**
 * Reads of a card's row where the latest one asked for is the one
 * that counts. A Continue card reads its row again for each answer that
 * can put its numbering right — its match, its probe, a retried probe —
 * and those reads can answer out of order.
 */

/** `read`, keyed: an answer to a call a later call for the same key
 *  has superseded resolves to null, as a row that cannot be read does. */
export function latestReads<T>(
	read: (key: string) => Promise<T | null>
): (key: string) => Promise<T | null> {
	return read;
}
