// Tile-grid entrance / exit transitions used by /anime/[id] and /play/[id]
// to make ep tiles fade up + scale + de-blur into place under cubicOut. With a
// per-index delay this gives a staggered "settle" feel as a page of tiles
// lands. Under reduced motion both stand still (see `reducible`): no
// duration and no stagger, so a page of tiles is simply there.
//
// css(t, u): for `in:`, t goes 0→1 and u = 1−t. So at t=0 the tile starts at
// opacity 0, scaled to 0.9, translated +28px below its final position, and
// blurred by 8px; it eases out to its rest state by t=1. settleOut mirrors
// the easing curve on the way out (drop + fade + soft blur), and uses a
// shorter duration so old tiles clear before new ones finish settling.

import { cubicOut } from 'svelte/easing';

import { reducible } from './transitions';

interface SettleOpts {
	delay?: number;
	duration?: number;
}

export const settle = reducible(
	(_node: Element, { delay = 0, duration = 620 }: SettleOpts = {}) => ({
		delay,
		duration,
		easing: cubicOut,
		css: (t: number, u: number) =>
			`opacity: ${t}; transform: translateY(${u * 28}px) scale(${0.9 + t * 0.1}); filter: blur(${u * 8}px);`
	})
);

export const settleOut = reducible(
	(_node: Element, { delay = 0, duration = 320 }: SettleOpts = {}) => ({
		delay,
		duration,
		easing: cubicOut,
		// For `out:` Svelte runs t from 1→0; u = 1−t. Mirror the in: shape
		// but drop upward (negative translateY) so it doesn't feel like the
		// same gesture playing in reverse.
		css: (t: number, u: number) =>
			`opacity: ${t}; transform: translateY(${u * -16}px) scale(${0.94 + t * 0.06}); filter: blur(${u * 4}px);`
	})
);
