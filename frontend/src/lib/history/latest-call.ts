/**
 * Reads of a card's row where the latest one asked for is the one
 * that counts. A Continue card reads its row again for each answer that
 * can put its numbering right — its match, its probe, a retried probe —
 * and those reads can answer out of order.
 */

/** `read`, keyed: an answer to a call a later call for the same key
 *  has superseded resolves to null, as a row that cannot be read does;
 *  a call that fails rejects as `read` did, superseded or not. */
export function latestReads<T>(
	read: (key: string) => Promise<T | null>
): (key: string) => Promise<T | null> {
	const asked = new Map<string, number>();
	return async (key) => {
		const call = (asked.get(key) ?? 0) + 1;
		asked.set(key, call);
		const answer = await read(key);
		return asked.get(key) === call ? answer : null;
	};
}
