/**
 * When a pending tip may show on screen. The backend decides which
 * tips are pending (`GET /api/onboarding`); this decides, from what is
 * on screen, whether one of them shows now. Rendering: every input is
 * the screen's.
 *
 * A tip that arrives (`moment: 'arrive'`) shows only:
 * - on its own page (or any page, for the app's chrome), never in the
 *   player;
 * - outside fullscreen, with none of the app's dialogs open;
 * - when no tip has closed on this page visit yet (one per visit);
 * - once its first place has been on screen and still for SETTLE_MS:
 *   at least half of it, or 160px of a tall one, below the top bar.
 */
import type { OnboardingSurface, OnboardingTip } from '$lib/api';

/** A place must be on screen and still this long before it is pointed at. */
export const SETTLE_MS = 600;
/** A press this soon after a balloon appears was aimed at the page. */
export const PRESS_GRACE_MS = 400;
/** A place this much on screen counts, however tall it is. */
export const TALL_SHOWN_PX = 160;

/** The page a SvelteKit route id is, for the tips' purposes; `other`
 *  for a page no tip lives on. */
export function surfaceOf(routeId: string | null): OnboardingSurface | 'other' {
	if (!routeId) return 'other';
	if (routeId === '/') return 'home';
	if (routeId.startsWith('/play')) return 'play';
	if (routeId === '/anime/[id]/franchise') return 'franchise';
	if (routeId.startsWith('/anime/[id]')) return 'show';
	if (routeId.startsWith('/settings')) return 'settings';
	return 'other';
}

/** Whether a place's box is on screen enough to point at: half of it,
 *  or TALL_SHOWN_PX of a tall one, between the top bar and the window's
 *  bottom, and some of its width. */
export function onScreen(
	box: { top: number; bottom: number; width: number },
	view: { h: number; top: number }
): boolean {
	const height = box.bottom - box.top;
	const shown = Math.min(box.bottom, view.h) - Math.max(box.top, view.top);
	return box.width > 0 && shown > 0 && (shown >= height / 2 || shown >= TALL_SHOWN_PX);
}

export interface ScreenNow {
	surface: OnboardingSurface | 'other';
	fullscreen: boolean;
	/** One of the app's own dialogs holds the page. */
	modalOpen: boolean;
	/** A tip already closed on this page visit. */
	closedThisVisit: boolean;
	/** Whether a place has been on screen and still for SETTLE_MS. */
	settled: (place: string) => boolean;
}

/** Whether `tip` may arrive on `surface`. */
export function arrivesOn(tip: OnboardingTip, surface: OnboardingSurface | 'other'): boolean {
	if (tip.moment !== 'arrive' || surface === 'play' || surface === 'other') return false;
	return tip.surface === 'any' || tip.surface === surface;
}

/** The pending tips whose places this page should watch, in order. */
export function watchedHere(
	tips: readonly OnboardingTip[],
	surface: OnboardingSurface | 'other'
): OnboardingTip[] {
	return tips.filter((t) => t.pending && t.places.length > 0 && arrivesOn(t, surface));
}

/** The one tip to show now, or none: the first pending one that arrives
 *  here whose first place has settled on screen. */
export function arrivingNow(tips: readonly OnboardingTip[], now: ScreenNow): OnboardingTip | null {
	if (now.fullscreen || now.modalOpen || now.closedThisVisit) return null;
	return watchedHere(tips, now.surface).find((t) => now.settled(t.places[0])) ?? null;
}

/** Whether a tip asked for on demand may show now: on its own page,
 *  outside fullscreen and dialogs, once its first place has settled.
 *  The one-per-visit rule and the seen list do not apply to it. */
export function armedNow(
	tip: OnboardingTip | null,
	now: Omit<ScreenNow, 'closedThisVisit'>
): OnboardingTip | null {
	if (!tip || now.fullscreen || now.modalOpen || tip.places.length === 0) return null;
	if (!arrivesOn({ ...tip, pending: true }, now.surface)) return null;
	return now.settled(tip.places[0]) ? tip : null;
}
