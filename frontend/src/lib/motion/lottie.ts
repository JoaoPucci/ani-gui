// Placeholder for the motion-aware Lottie: the signatures its tests
// need to type-check, and nothing behind them yet.
/* eslint-disable @typescript-eslint/no-unused-vars -- placeholder signatures the next commit fills in */
import type { ReducedMotion } from './reduced-motion.svelte';

export interface LottiePlayer {
	readonly totalFrames: number;
	play(): void;
	pause(): void;
	goToAndStop(value: number, isFrame: boolean): void;
	destroy(): void;
}

type Preference = Pick<ReducedMotion, 'current' | 'onChange'>;

export function stillFrame(_totalFrames: number): number {
	throw new Error('not implemented');
}

export function followMotionPreference(_player: LottiePlayer, _preference: Preference): () => void {
	throw new Error('not implemented');
}

export async function playLottie(
	_container: Element,
	_animationData: unknown,
	_preference?: Preference
): Promise<{ destroy(): void }> {
	throw new Error('not implemented');
}
