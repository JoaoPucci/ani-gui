// The loading overlay's Lottie: it plays while motion is on and holds
// one frame under reduced motion, following a change made while it
// is on screen.
import { describe, expect, it, vi } from 'vitest';

const loadAnimation = vi.hoisted(() => vi.fn());
vi.mock('lottie-web', () => ({ default: { loadAnimation } }));

import { followMotionPreference, playLottie, stillFrame, type LottiePlayer } from './lottie';

function fakePlayer(totalFrames = 76) {
	const calls: string[] = [];
	const player: LottiePlayer = {
		totalFrames,
		play: () => calls.push('play'),
		pause: () => calls.push('pause'),
		goToAndStop: (value, isFrame) => calls.push(`stop:${value}:${isFrame}`),
		destroy: () => calls.push('destroy')
	};
	return { player, calls };
}

function fakePreference(reduced: boolean) {
	const listeners = new Set<(r: boolean) => void>();
	return {
		current: reduced,
		onChange(listener: (r: boolean) => void) {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
		flip(next: boolean) {
			this.current = next;
			for (const l of listeners) l(next);
		}
	};
}

describe('stillFrame', () => {
	it('holds the middle of the loop', () => {
		expect(stillFrame(76)).toBe(38);
		expect(stillFrame(1)).toBe(0);
		expect(stillFrame(0)).toBe(0);
	});
});

describe('followMotionPreference', () => {
	it('plays while motion is on', () => {
		const { player, calls } = fakePlayer();
		followMotionPreference(player, fakePreference(false));
		expect(calls).toEqual(['play']);
	});

	it('holds a frame under reduced motion and never plays', () => {
		const { player, calls } = fakePlayer();
		followMotionPreference(player, fakePreference(true));
		expect(calls).toEqual(['stop:38:true']);
	});

	it('follows a change until it is let go', () => {
		const { player, calls } = fakePlayer();
		const preference = fakePreference(false);
		const stop = followMotionPreference(player, preference);
		preference.flip(true);
		preference.flip(false);
		stop();
		preference.flip(true);
		expect(calls).toEqual(['play', 'stop:38:true', 'play']);
	});
});

describe('playLottie', () => {
	it('loads without autoplay and lets the preference decide', async () => {
		const { player, calls } = fakePlayer();
		loadAnimation.mockReturnValueOnce(player);
		const container = {} as Element;
		const handle = await playLottie(container, { op: 76 }, fakePreference(true));
		expect(loadAnimation).toHaveBeenCalledWith({
			container,
			renderer: 'svg',
			loop: true,
			autoplay: false,
			animationData: { op: 76 }
		});
		expect(calls).toEqual(['stop:38:true']);
		handle.destroy();
		expect(calls).toEqual(['stop:38:true', 'destroy']);
	});

	it('stops following the preference once destroyed', async () => {
		const { player, calls } = fakePlayer();
		loadAnimation.mockReturnValueOnce(player);
		const preference = fakePreference(false);
		const handle = await playLottie({} as Element, {}, preference);
		handle.destroy();
		preference.flip(true);
		expect(calls).toEqual(['play', 'destroy']);
	});
});

describe('playLottie without a preference', () => {
	it('follows the app preference', async () => {
		const { player, calls } = fakePlayer();
		loadAnimation.mockReturnValueOnce(player);
		const handle = await playLottie({} as Element, {});
		// node has no window: the app reads full motion.
		expect(calls).toEqual(['play']);
		handle.destroy();
	});
});
