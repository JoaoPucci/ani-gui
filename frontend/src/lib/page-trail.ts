/**
 * The topbar trail's two writers: a page naming itself, and the layout
 * resetting the trail after each navigation.
 */

import type { Writable } from 'svelte/store';
import { breadcrumb, defaultTrailFor, type BreadcrumbSegment } from '$lib/breadcrumb';

/** A page's own trail, and the address of the page it names. */
export interface PageTrail {
	path: string;
	trail: BreadcrumbSegment[];
}

/** The trail on screen once the layout has reset it for a navigation
 *  that landed on `path`. */
export function trailAfterReset(
	_own: PageTrail | null,
	_path: string,
	routeId: string | null
): BreadcrumbSegment[] {
	return defaultTrailFor(routeId);
}

/** Who writes the trail: a page naming itself, and the layout
 *  resetting it after each navigation. */
export interface TrailKeeper {
	/** The page at `path` (a pathname) names itself with `trail`. */
	name(path: string, trail: BreadcrumbSegment[]): void;
	/** A navigation landed on `path`, a page of route `routeId`. */
	reset(path: string, routeId: string | null): void;
}

export function createTrailKeeper(store: Writable<BreadcrumbSegment[]>): TrailKeeper {
	return {
		name: (_path, trail) => store.set(trail),
		reset: (path, routeId) => store.set(trailAfterReset(null, path, routeId))
	};
}

/** The app's one keeper of the topbar trail. */
export const pageTrail = createTrailKeeper(breadcrumb);
