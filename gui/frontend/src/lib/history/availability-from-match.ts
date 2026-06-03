import {
	altTitlesFromKitsu,
	yearFromKitsuRef,
	type AvailabilityArgs,
	type AvailabilityResponse,
	type KitsuAnimeRef
} from '$lib/api';

/**
 * Builds the loader's `fetchAvailability` dep from a low-level
 * `checkAvailability` IPC. The returned function maps a Kitsu match
 * into the `AvailabilityArgs` shape the backend expects — same shape
 * the detail page's mount-time probe uses, so the title-match
 * disambiguation has identical context whether the call originates
 * from home's cache-miss fallback or from /anime/[id]'s mount probe.
 *
 * Curried so the home page can hand a single value to
 * `loadContinueWatchingState` without keeping its own closure: the
 * closure that would otherwise live on +page.svelte (and therefore
 * outside any test surface) becomes a definition in this lib file,
 * exercised by the unit tests below.
 */
export function makeFetchAvailability(
	checkAvailabilityFn: (args: AvailabilityArgs) => Promise<AvailabilityResponse>
): (match: KitsuAnimeRef, mode: 'sub' | 'dub') => Promise<AvailabilityResponse> {
	return (match, mode) =>
		checkAvailabilityFn({
			title: match.canonical_title,
			mode,
			alt_titles: altTitlesFromKitsu(match),
			episode_count: match.episode_count ?? undefined,
			year: yearFromKitsuRef(match) ?? undefined,
			kitsu_id: match.id,
			status: match.status ?? undefined
		});
}
