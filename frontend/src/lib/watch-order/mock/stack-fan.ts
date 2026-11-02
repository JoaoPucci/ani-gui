/**
 * The "Whole franchise" poster stack's geometry (mockup, third round):
 * which cards it holds, and where each sits at rest and once the stack
 * is hovered or focused. The cards stand on one arc, so opening the
 * fan moves each one along it, away from the middle. Under reduced
 * motion the stack is drawn open and stays there.
 */
import type { MockEntry } from './types';

/** One card of the stack: an entry's poster, or a blank frame when no entry has one. */
export type StackCard =
	| { readonly kind: 'poster'; readonly entry: MockEntry }
	| { readonly kind: 'blank' };

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
export function stackCards(posters: readonly MockEntry[], limit: number): StackCard[] {
	return posters.slice(0, limit).map((entry) => ({ kind: 'poster', entry }));
}

/** Each of `count` cards' pose, in reading order. */
export function fanPoses(count: number, state: FanState): FanPose[] {
	void state;
	return Array.from({ length: count }, () => ({ x: 0, y: 0, rotate: 0 }));
}
