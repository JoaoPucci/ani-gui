/**
 * The behaviour a scroll the app makes for the user should take: a
 * glide with motion on, a jump under reduced motion.
 *
 * `scroll-behavior` in CSS is held to `auto` under reduced motion by
 * `tokens.css`, but a `behavior` passed to `scrollBy`,
 * `scrollIntoView` or `scrollTo` overrides it, so a scripted scroll
 * takes its behaviour from here.
 */
import { reducedMotion } from './reduced-motion.svelte';

export function behaviorFor(reduced: boolean): ScrollBehavior {
	return reduced ? 'auto' : 'smooth';
}

/** The behaviour for a scroll starting now. */
export function scrollBehavior(): ScrollBehavior {
	return behaviorFor(reducedMotion.current);
}
