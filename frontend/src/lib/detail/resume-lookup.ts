/**
 * The detail page's resume lookup: the history row the page's Kitsu
 * entry resumes from, or `null` when none does.
 */
export function lookupResume<T>(
	kitsuId: string,
	lookup: (kitsuId: string) => Promise<T | null>,
	// eslint-disable-next-line @typescript-eslint/no-unused-vars
	_detailServed: Promise<unknown>
): Promise<T | null> {
	return lookup(kitsuId);
}
