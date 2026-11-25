/**
 * The player's side of the load tip: which tip it shows, whether an
 * Escape is the tip's to take, and where focus goes when the tip leaves
 * with focus in it. Rendering glue.
 */
import type { OnboardingTip } from '$lib/api';
import { isModalOpen } from '$lib/components/modal-layer';

/** The tip the player shows while a stream loads: one asked for from
 *  the Debug page, else the first pending tip that loads in the
 *  player. Whether it was asked for decides whether closing it is
 *  seen. */
export function playerTip(
	tips: readonly OnboardingTip[],
	armed: OnboardingTip | null
): { tip: OnboardingTip; asked: boolean } | null {
	if (armed?.moment === 'stream-load') return { tip: armed, asked: true };
	const tip = tips.find((t) => t.pending && t.moment === 'stream-load' && t.surface === 'play');
	return tip ? { tip, asked: false } : null;
}

/** Whether an Escape is the load tip's: not when something else took
 *  it, a dialog holds the page, or a menu or popover is open (the
 *  player's own, the downloads list, an account menu), which take it
 *  first. */
export function escapeIsTips(e: KeyboardEvent, playerMenuOpen: boolean, doc: Document): boolean {
	if (e.key !== 'Escape' || e.defaultPrevented || playerMenuOpen || isModalOpen()) return false;
	return doc.querySelector('[aria-expanded="true"]') === null;
}

/** Focus the player's first control that can take it: the error
 *  panel's action, else a control button, else the frame itself. */
export function focusPlayer(frame: HTMLElement): void {
	const candidates = [
		...frame.querySelectorAll<HTMLElement>(
			'.player-error-action, .pc-buttons-row .pc-btn:not([disabled])'
		)
	].filter((el) => !el.closest('[inert]'));
	for (const el of candidates) {
		el.focus({ preventScroll: true });
		if (frame.ownerDocument.activeElement === el) return;
	}
	if (!frame.hasAttribute('tabindex')) frame.setAttribute('tabindex', '-1');
	frame.focus({ preventScroll: true });
}
