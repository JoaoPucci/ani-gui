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
