/**
 * The strip's link to the franchise page, a stack of posters: which
 * cards it holds, and where each sits at rest and once the stack
 * is hovered or focused. The cards stand on one arc, so opening the
 * fan moves each one along it, away from the middle: the stack draws
 * a card by rotating it about the arc's centre, `FAN_RADIUS_REM` below
 * the fan's foot, so the transition between the two poses follows the
 * arc rather than cutting across it. Under reduced motion the stack
 * is drawn open and stays there.
 */
import type { WatchEntry } from './types';

/** One card of the stack: an entry's poster, or a blank frame when no entry has one. */
export type StackCard =
	| { readonly kind: 'poster'; readonly entry: WatchEntry }
	| { readonly kind: 'blank' };

/** Fanned, neighbouring cards stand this many degrees apart. */
const OPEN_STEP = 9;
/** At rest, a little closer together on the same arc. */
const REST_STEP = 6.5;
/** How far a lone card rises when the stack is hovered, in rem. */
const LONE_LIFT = 0.25;

/**
 * The radius, in rem, of the arc a two- or three-card fan stands on:
 * fanned, three cards' outer feet sit 1.3125rem from the middle, where
 * the second round's hovered fan put them.
 */
export const FAN_RADIUS_REM = 1.3125 / Math.sin((OPEN_STEP * Math.PI) / 180);

/** At rest, or fanned out (hover, focus, reduced motion). */
export type FanState = 'rest' | 'open';

/**
 * A card's place, from the middle of the fan to the middle of the
 * card's foot: `x` and `y` in rem (y grows downwards), `rotate` in
 * degrees, clockwise.
 */
export interface FanPose {
	readonly x: number;
	readonly y: number;
	readonly rotate: number;
}

/** The cards: up to `limit` of `posters`, or three blank frames when there are none. */
export function stackCards(posters: readonly WatchEntry[], limit: number): StackCard[] {
	const shown = posters.slice(0, Math.max(0, limit));
	if (shown.length === 0) return [{ kind: 'blank' }, { kind: 'blank' }, { kind: 'blank' }];
	return shown.map((entry) => ({ kind: 'poster', entry }));
}

/** Each of `count` cards' pose, in reading order. */
export function fanPoses(count: number, state: FanState): FanPose[] {
	if (count === 1) return [{ x: 0, y: state === 'open' ? -LONE_LIFT : 0, rotate: 0 }];
	const step = state === 'open' ? OPEN_STEP : REST_STEP;
	return Array.from({ length: count }, (_, i) => {
		const rotate = (i - (count - 1) / 2) * step;
		const rad = (rotate * Math.PI) / 180;
		return {
			x: FAN_RADIUS_REM * Math.sin(rad),
			y: FAN_RADIUS_REM * (1 - Math.cos(rad)),
			rotate
		};
	});
}

/** A card's width, in rem. */
export const CARD_INLINE_REM = 2.3;
/** A card's height, in rem: the poster's 5:7. */
export const CARD_BLOCK_REM = (CARD_INLINE_REM * 7) / 5;

/** The fan's box, in rem, and how far above its bottom edge the cards' feet stand. */
export interface FanBox {
	readonly inline: number;
	readonly block: number;
	readonly foot: number;
}

/** The box every card of a fan stays inside, at rest and fanned out. */
export function fanBox(): FanBox {
	return { inline: 5.5, block: 3.4, foot: 0.1 };
}
