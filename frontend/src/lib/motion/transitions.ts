// Placeholder for the motion-aware transitions: the signatures their
// tests need to type-check, and nothing behind them yet.
/* eslint-disable @typescript-eslint/no-unused-vars -- placeholder signatures the next commit fills in */
import {
	fade as svelteFade,
	fly as svelteFly,
	scale as svelteScale,
	slide as svelteSlide
} from 'svelte/transition';
import { flip as svelteFlip } from 'svelte/animate';

interface Timed {
	duration?: number;
	delay?: number;
}

export function stillWhenReduced<C extends Timed | (() => Timed)>(config: C, _reduced: boolean): C {
	return config;
}

export function reducible<A extends unknown[], C extends Timed | (() => Timed)>(
	transition: (node: Element, ...args: A) => C
): (node: Element, ...args: A) => C {
	return transition;
}

export const fade = reducible(svelteFade);
export const fly = reducible(svelteFly);
export const scale = reducible(svelteScale);
export const slide = reducible(svelteSlide);
export const flip = reducible(svelteFlip);
