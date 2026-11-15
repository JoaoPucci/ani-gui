/**
 * The Kitsu subtypes the search page offers as type filters: each
 * labelled subtype search can answer. None is offered for music: the
 * backend drops music videos from every search (Kitsu lists far more
 * of them than the provider serves), so that filter could only empty
 * the grid.
 */
export const SEARCH_TYPE_FILTERS: readonly string[] = ['TV', 'movie', 'special', 'OVA', 'ONA'];

/** The filters among `saved` the page still offers, in their order. */
export function offeredTypeFilters(saved: readonly string[]): string[] {
	return saved.filter((s) => SEARCH_TYPE_FILTERS.includes(s));
}
