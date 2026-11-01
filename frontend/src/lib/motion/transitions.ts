/**
 * Svelte's transitions and FLIP, made to stand still under reduced
 * motion.
 *
 * Every `transition:`, `in:`, `out:` and `animate:` directive in the
 * app names a function from here, or one built with `reducible`; the
 * guard in `tests/lint/reduced-motion.test.ts` fails on any other.
 * Under reduced motion the config a transition returns loses its
 * duration and its delay, and Svelte, handed a transition with neither,
 * moves the element straight to its end state without starting an
 * animation.
 */
import {
	fade as svelteFade,
	fly as svelteFly,
	scale as svelteScale,
	slide as svelteSlide
} from 'svelte/transition';
import { flip as svelteFlip } from 'svelte/animate';

import { reducedMotion } from './reduced-motion.svelte';

interface Timed {
	duration?: number;
	delay?: number;
}

/** `config` as it is with motion on; with no time in it under reduced
 *  motion. A deferred config (a function, as `crossfade` returns) is
 *  stilled when Svelte calls it. */
export function stillWhenReduced<C extends Timed | (() => Timed)>(config: C, reduced: boolean): C {
	if (!reduced) return config;
	if (typeof config === 'function') return (() => stillWhenReduced(config(), true)) as C;
	return { ...config, duration: 0, delay: 0 };
}

/** Wraps a transition or animation function so it reads the motion
 *  preference each time it starts. Use it for a directive the app
 *  builds itself:
 *
 *      const cwOutScale = reducible((node: Element) => scale(node, { … }));
 */
export function reducible<A extends unknown[], C extends Timed | (() => Timed)>(
	transition: (node: Element, ...args: A) => C
): (node: Element, ...args: A) => C {
	return (node, ...args) => stillWhenReduced(transition(node, ...args), reducedMotion.current);
}

export const fade = reducible(svelteFade);
export const fly = reducible(svelteFly);
export const scale = reducible(svelteScale);
export const slide = reducible(svelteSlide);
export const flip = reducible(svelteFlip);
