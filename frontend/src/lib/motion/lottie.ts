/**
 * The loading overlay's Lottie, played under the motion preference.
 *
 * Lottie runs its own frame loop, which no stylesheet reaches, so the
 * animation is loaded without autoplay and this module decides: it
 * plays while motion is on and holds the middle of its loop under
 * reduced motion — still recognisably the loader, not moving. A change
 * to the preference while the overlay is up is followed.
 *
 * lottie-web is imported lazily here, as the overlay always did, so its
 * runtime only loads when an overlay first shows.
 */
import { reducedMotion, type ReducedMotion } from './reduced-motion.svelte';

/** The part of a Lottie `AnimationItem` this module drives. */
export interface LottiePlayer {
	readonly totalFrames: number;
	play(): void;
	pause(): void;
	goToAndStop(value: number, isFrame: boolean): void;
	destroy(): void;
}

type Preference = Pick<ReducedMotion, 'current' | 'onChange'>;

/** The frame a still loader holds. */
export function stillFrame(totalFrames: number): number {
	return Math.floor(totalFrames / 2);
}

/** Plays or holds `player` by the preference, now and on each change.
 *  Returns the unsubscribe. */
export function followMotionPreference(player: LottiePlayer, preference: Preference): () => void {
	const apply = (reduced: boolean) => {
		if (reduced) player.goToAndStop(stillFrame(player.totalFrames), true);
		else player.play();
	};
	apply(preference.current);
	return preference.onChange(apply);
}

/** Loads `animationData` into `container`, looping, under the motion
 *  preference. `destroy` stops following it and tears the player down. */
export async function playLottie(
	container: Element,
	animationData: unknown,
	preference: Preference = reducedMotion
): Promise<{ destroy(): void }> {
	const lottie = (await import('lottie-web')).default;
	const player = lottie.loadAnimation({
		container,
		renderer: 'svg',
		loop: true,
		autoplay: false,
		animationData
	}) as unknown as LottiePlayer;
	const unfollow = followMotionPreference(player, preference);
	return {
		destroy() {
			unfollow();
			player.destroy();
		}
	};
}
