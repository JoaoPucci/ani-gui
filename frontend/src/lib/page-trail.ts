/**
 * The topbar trail's two writers, made independent of their order: a
 * page naming itself, and the layout resetting the trail after each
 * navigation. See createTrailKeeper.
 */

import type { Writable } from 'svelte/store';
import { breadcrumb, defaultTrailFor, type BreadcrumbSegment } from '$lib/breadcrumb';

/** A page's own trail, and the address of the page it names. */
export interface PageTrail {
	path: string;
	trail: BreadcrumbSegment[];
}

/** The trail on screen once the layout has reset it for a navigation
 *  that landed on `path`: the page's own when the page at that address
 *  has named itself, else the default for the route. */
export function trailAfterReset(
	own: PageTrail | null,
	path: string,
	routeId: string | null
): BreadcrumbSegment[] {
	return own !== null && own.path === path ? own.trail : defaultTrailFor(routeId);
}

/** Who writes the trail: a page naming itself, and the layout
 *  resetting it after each navigation. */
export interface TrailKeeper {
	/** The page at `path` (a pathname) names itself with `trail`. */
	name(path: string, trail: BreadcrumbSegment[]): void;
	/** A navigation landed on `path`, a page of route `routeId`. */
	reset(path: string, routeId: string | null): void;
}

/**
 * The two writers of `store`, made independent of their order.
 *
 * SvelteKit reports a navigation to the layout only after the page it
 * lands on has rendered. A navigation that stays on the page's own
 * address — the same show picked again from the topbar search, the
 * episode pager's `?page=`, the player's switch to another episode —
 * keeps the page mounted, and the page has already named itself (or
 * names nothing new) by the time the layout resets. Nothing orders a
 * fresh page's naming against the reset either: the page names itself
 * when its data lands. The keeper remembers which address a page's
 * trail belongs to, and a reset for that address keeps it. A
 * navigation to another address drops it, so a show's title never
 * lingers on the page after it.
 */
export function createTrailKeeper(store: Writable<BreadcrumbSegment[]>): TrailKeeper {
	let own: PageTrail | null = null;
	return {
		name(path, trail) {
			own = { path, trail };
			store.set(trail);
		},
		reset(path, routeId) {
			if (own !== null && own.path !== path) own = null;
			store.set(trailAfterReset(own, path, routeId));
		}
	};
}

/** The app's one keeper of the topbar trail. */
export const pageTrail = createTrailKeeper(breadcrumb);
