/**
 * The franchise page's topbar trail. Everything else the page draws is
 * the backend's answer (`$lib/api` `FranchiseWire`).
 */
import type { FranchiseWire } from '$lib/api';
import type { BreadcrumbSegment } from '$lib/breadcrumb';
import { m } from '$lib/paraglide/messages';

/**
 * The topbar trail for the franchise page: Home, the show it was
 * opened from (a link back to it), where the page lists it, then
 * "Franchise".
 */
export function franchiseTrail(page: Pick<FranchiseWire, 'opened_from'>): BreadcrumbSegment[] {
	const from = page.opened_from;
	return [
		{ label: m.breadcrumb_home(), href: '/' },
		...(from ? [{ label: from.title, href: `/anime/${from.id}` }] : []),
		{ label: m.watchorder_franchise_eyebrow() }
	];
}
