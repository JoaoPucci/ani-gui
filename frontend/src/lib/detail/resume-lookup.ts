/**
 * The detail page's resume lookup: the history row the page's Kitsu
 * entry resumes from, or `null` when none does.
 *
 * The page runs it beside its Kitsu detail fetch rather than after,
 * so a row shows without waiting on the network. A fetch that serves
 * an entry Kitsu once answered gone clears that mark, and the lookup,
 * being local, has usually answered by then — with no row, since the
 * mark hid it. So an empty answer is asked again once the detail is
 * served. The page cannot tell whether the fetch cleared a mark, so
 * every empty answer is asked twice — for a show never watched, one
 * more lookup of the local history. A found row is final, and a
 * failed fetch cleared nothing.
 */
export async function lookupResume<T>(
	kitsuId: string,
	lookup: (kitsuId: string) => Promise<T | null>,
	detailServed: Promise<unknown>
): Promise<T | null> {
	const first = await lookup(kitsuId);
	if (first !== null) return first;
	try {
		await detailServed;
	} catch {
		return null;
	}
	return lookup(kitsuId);
}
