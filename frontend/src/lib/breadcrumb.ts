/**
 * Breadcrumb path store — drives the Home › Anime › Episode trail
 * shown in the topbar. Replaces the bare BackButton: each segment is
 * a clickable hop to that level of the hierarchy, and the current
 * page renders as plain text.
 *
 * Default contributors:
 *  - The layout sets a generic two-segment trail on every navigation
 *    based on URL alone (so a hard-load lands with something visible).
 *  - Routes that have richer labels (anime title, episode number)
 *    overwrite the trail in onMount once their data loads.
 *
 * On navigation away the layout resets to defaults; this avoids a
 * stale "Naruto Shippuuden" segment lingering on /search after the
 * user navigated away.
 */

import { writable } from 'svelte/store';
import { m } from '$lib/paraglide/messages';

export interface BreadcrumbSegment {
	label: string;
	/** When omitted, the segment renders as the current page (plain
	 *  text, non-clickable). The last segment in a trail must always
	 *  be `href`-less. */
	href?: string;
}

export const breadcrumb = writable<BreadcrumbSegment[]>([]);

/** Build the default trail from a route id alone. Used by the layout
 *  on every navigation so a page with no breadcrumb hook still shows
 *  something meaningful. Routes override this on mount with richer
 *  labels (anime title, episode number). Labels resolve through
 *  Paraglide so the trail re-renders in the active locale. */
export function defaultTrailFor(routeId: string | null): BreadcrumbSegment[] {
	if (!routeId || routeId === '/') {
		return [{ label: m.breadcrumb_home() }];
	}
	const home: BreadcrumbSegment = { label: m.breadcrumb_home(), href: '/' };
	if (routeId.startsWith('/search')) return [home, { label: m.breadcrumb_search() }];
	if (routeId.startsWith('/account')) return [home, { label: m.breadcrumb_account() }];
	if (routeId.startsWith('/settings')) return [home, { label: m.breadcrumb_settings() }];
	if (routeId.startsWith('/diagnostics')) return [home, { label: m.breadcrumb_diagnostics() }];
	if (routeId.startsWith('/about')) return [home, { label: m.breadcrumb_about() }];
	if (routeId.startsWith('/anime')) return [home, { label: m.breadcrumb_anime() }];
	if (routeId.startsWith('/play')) return [home, { label: m.breadcrumb_watching() }];
	return [home];
}

/** `origin` when it is a search the reader ran (the search page with a
 *  query on it), else null: the one page behind that the breadcrumb
 *  names and steps back to. */
export function searchOrigin(origin: string | null): string | null {
	if (!origin) return null;
	const url = new URL(origin, 'http://app.invalid');
	const ran = url.pathname.endsWith('/search') && (url.searchParams.get('q') ?? '').trim() !== '';
	return ran ? origin : null;
}

/**
 * The trail with the search the page was opened from named between
 * Home and the page, so the reader can see where back leads and get
 * there from the topbar. `origin` is the address of the page behind
 * the current one ($lib/history/nav-depth); a trail that already names
 * it, the search page's own trail, and a trail with no parent are left
 * as they are.
 */
export function withSearchOrigin(
	trail: BreadcrumbSegment[],
	origin: string | null,
	routeId: string | null
): BreadcrumbSegment[] {
	const search = searchOrigin(origin);
	if (!search || trail.length < 2 || routeId?.startsWith('/search')) return trail;
	if (trail.some((s) => s.href === search)) return trail;
	return [trail[0], { label: m.breadcrumb_search(), href: search }, ...trail.slice(1)];
}

/** Whether following `href` is stepping back to the page behind: it
 *  is that page's address. Stepping back returns to the history entry
 *  as it was left, where opening the address would start it afresh. */
export function goesBack(href: string, back: string | null): boolean {
	return back !== null && href === back;
}

/** Characters that draw two cells wide in a monospace face: the East
 *  Asian wide and full-width blocks (Hangul jamo, CJK and its
 *  punctuation, kana, Hangul syllables, compatibility ideographs,
 *  full-width forms, and the supplementary ideograph planes). */
const WIDE =
	/[\u1100-\u115F\u2E80-\u303E\u3041-\u33FF\u3400-\u4DBF\u4E00-\u9FFF\uA000-\uA4CF\uAC00-\uD7A3\uF900-\uFAFF\uFE30-\uFE4F\uFF00-\uFF60\uFFE0-\uFFE6\u{20000}-\u{3FFFD}]/u;

/** How many monospace cells `label` takes: one per code point, two
 *  for a wide one. */
export function labelCells(label: string): number {
	void label;
	void WIDE;
	return 0;
}

/** The segment that gives way when the topbar runs short of room: the
 *  one whose label is widest (labelCells) — in practice the show's
 *  title — so the short ones (Home, Search, the episode) stay readable
 *  while it ellipsises. Of equally wide labels, the last, nearest the
 *  page. -1 for an empty trail. */
export function yieldingSegment(trail: BreadcrumbSegment[]): number {
	void trail;
	return -1;
}
