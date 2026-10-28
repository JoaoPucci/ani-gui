// The motion of a card joining or leaving a horizontal strip, and of the
// cards beside it closing or opening the gap: the Continue Watching
// strip's delete animation, shared so every strip that gains or loses a
// card moves the same way. A card leaving shrinks and fades out
// (`cardScale` as an `out:`); one arriving is the same transition run
// forward (`cardScale` as an `in:`); the cards around it slide to their
// new places (`cardShift` as an `animate:`). Which cards move at all is
// the caller's gate — `$lib/history/animation-gate` — so a strip that
// fills or re-sorts on load does not animate. Under prefers-reduced-motion
// nothing moves: the strip shows its new state at once.

import { scale, type TransitionConfig } from 'svelte/transition';
import { flip, type AnimationConfig } from 'svelte/animate';
import { quintOut } from 'svelte/easing';

/** A card shrinking out of, or growing into, its place. */
export const CARD_SCALE = { duration: 240, start: 0.6, opacity: 0, easing: quintOut } as const;

/** The cards beside it sliding to close or open the gap. */
export const CARD_SHIFT = { duration: 280, easing: quintOut } as const;

/** No motion: what a card the gate does not let move gets. */
export const STILL: TransitionConfig = { duration: 0 };

/** Whether the user asked for less motion: then every card holds still
 *  and the strip simply shows its new state. */
function reducedMotion(): boolean {
	return (
		typeof window !== 'undefined' &&
		!!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches
	);
}

/** The scale transition of a card leaving or arriving. */
export function cardScale(
	node: Element,
	timing: Pick<TransitionConfig, 'duration' | 'easing'> = {}
): TransitionConfig {
	const config = { ...CARD_SCALE, ...timing };
	return scale(node, reducedMotion() ? { ...config, duration: 0 } : config);
}

/** The slide of a card whose place in the strip changed. */
export function cardShift(
	node: Element,
	animation: { from: DOMRect; to: DOMRect },
	timing: Pick<AnimationConfig, 'duration' | 'easing'> = {}
): AnimationConfig {
	const config = { ...CARD_SHIFT, ...timing };
	return flip(node, animation, reducedMotion() ? { ...config, duration: 0 } : config);
}
