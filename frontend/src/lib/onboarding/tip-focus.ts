/**
 * Where focus goes as a tip closes ($./closing names the route's
 * choice). Rendering glue.
 *
 * - `place`: the place itself if it takes focus, else its first control.
 * - `back`: where focus was before the tip, which the modal layer has
 *   given back; when that was nowhere (the page, or the balloon that is
 *   going), the place as above.
 * - `none` and `leave`: nothing moves it.
 */
const FOCUSABLE = 'a[href], button:not([disabled]), input, select, [tabindex]:not([tabindex="-1"])';

export function focusAfterClose(
	to: 'place' | 'back' | 'none' | 'leave',
	doc: Document,
	place: HTMLElement,
	balloon: HTMLElement | null
): void {
	if (to === 'none' || to === 'leave') return;
	const active = doc.activeElement;
	const nowhere = !active || active === doc.body || (balloon?.contains(active) ?? false);
	if (to === 'back' && !nowhere) return;
	const target = place.matches(FOCUSABLE) ? place : place.querySelector<HTMLElement>(FOCUSABLE);
	target?.focus({ preventScroll: true });
}
