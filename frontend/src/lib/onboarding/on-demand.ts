/**
 * When a tip asked for on demand may show: one the Debug page's "Show
 * it" armed. The gate's other rules ($./gate) apply, but for the
 * visit's one tip and the seen list. Rendering: every input is the
 * screen's.
 */
import type { OnboardingTip } from '$lib/api';
import { arrivesOn, type ScreenNow } from './gate';

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

/** Whether the Debug page can preview `tip`: one that arrives on a page
 *  of its own, or one that loads in the player (it then shows on the
 *  next slow start), with places. */
export function previewable(tip: OnboardingTip): boolean {
	if (tip.places.length === 0) return false;
	if (tip.moment === 'stream-load') return tip.surface === 'play';
	return tip.surface !== 'play';
}
