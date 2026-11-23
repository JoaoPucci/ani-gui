// @vitest-environment happy-dom
//
// The loading overlay's Lottie: it plays while motion is on and holds
// one frame under reduced motion, following a change made while it
// is on screen.
import fc from 'fast-check';
import { describe, expect, it, vi } from 'vitest';

const loadAnimation = vi.hoisted(() => vi.fn());
vi.mock('lottie-web/build/player/lottie_light', () => ({ default: { loadAnimation } }));

import {
	followMotionPreference,
	loaderAnimation,
	playLottie,
	stillFrame,
	type LottiePlayer
} from './lottie';

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

/** Loads that settle only when the test says so. Each player records
 *  the container it went into and whether it was torn down. */
function deferredLoads() {
	type Live = { container: Element; destroyed: boolean };
	const players: Live[] = [];
	const pending: { container: Element; settle: (ok: boolean) => void }[] = [];
	const load = (container: Element) =>
		new Promise<{ destroy(): void }>((resolve, reject) => {
			pending.push({
				container,
				settle: (ok) => {
					if (!ok) return reject(new Error('runtime failed to load'));
					const live: Live = { container, destroyed: false };
					players.push(live);
					resolve({ destroy: () => (live.destroyed = true) });
				}
			});
		});
	const settleNext = async (ok = true) => {
		pending.shift()?.settle(ok);
		await new Promise((r) => setTimeout(r, 0));
	};
	const live = () => players.filter((p) => !p.destroyed);
	return { load, pending, players, live, settleNext };
}

describe('loaderAnimation', () => {
	const box = () => document.createElement('div');

	it('loads into the container it is shown in and keeps the player', async () => {
		const d = deferredLoads();
		const anim = loaderAnimation(d.load);
		const c = box();
		anim.show(c);
		expect(d.pending.map((p) => p.container)).toEqual([c]);
		await d.settleNext();
		expect(d.live().map((p) => p.container)).toEqual([c]);
	});

	it('starts one load however often it is shown while one is on its way', async () => {
		const d = deferredLoads();
		const anim = loaderAnimation(d.load);
		const c = box();
		anim.show(c);
		anim.show(c);
		expect(d.pending).toHaveLength(1);
		await d.settleNext();
		anim.show(c);
		expect(d.pending).toHaveLength(0);
		expect(d.live()).toHaveLength(1);
	});

	it('tears the player down when hidden', async () => {
		const d = deferredLoads();
		const anim = loaderAnimation(d.load);
		anim.show(box());
		await d.settleNext();
		anim.hide();
		expect(d.live()).toEqual([]);
	});

	it('tears down a player that arrives after the overlay was hidden', async () => {
		const d = deferredLoads();
		const anim = loaderAnimation(d.load);
		anim.show(box());
		anim.hide();
		await d.settleNext();
		expect(d.players).toHaveLength(1);
		expect(d.live()).toEqual([]);
	});

	it('keeps only the player for the container shown last', async () => {
		const d = deferredLoads();
		const anim = loaderAnimation(d.load);
		const first = box();
		const second = box();
		anim.show(first);
		anim.hide();
		anim.show(second);
		await d.settleNext();
		await d.settleNext();
		expect(d.live().map((p) => p.container)).toEqual([second]);
	});

	it('shows nothing, and throws nothing, when the runtime fails to load', async () => {
		const d = deferredLoads();
		const anim = loaderAnimation(d.load);
		const c = box();
		anim.show(c);
		await d.settleNext(false);
		expect(d.live()).toEqual([]);
		// A later showing tries again.
		anim.show(c);
		expect(d.pending).toHaveLength(1);
	});

	it('leaves at most one live player, in the container last shown, for any sequence', async () => {
		const op = fc.oneof(
			fc.record({ kind: fc.constant('show' as const), which: fc.integer({ min: 0, max: 1 }) }),
			fc.record({ kind: fc.constant('hide' as const) }),
			fc.record({ kind: fc.constant('settle' as const), ok: fc.boolean() })
		);
		await fc.assert(
			fc.asyncProperty(fc.array(op, { maxLength: 12 }), async (ops) => {
				const d = deferredLoads();
				const anim = loaderAnimation(d.load);
				const boxes = [box(), box()];
				let shown: Element | null = null;
				// A load that failed after the box was last shown may have
				// been that box's, which leaves it without an animation.
				let failedSinceShown = false;
				for (const o of ops) {
					if (o.kind === 'show') {
						shown = boxes[o.which];
						failedSinceShown = false;
						anim.show(shown);
					} else if (o.kind === 'hide') {
						shown = null;
						anim.hide();
					} else {
						if (!o.ok && d.pending.length) failedSinceShown = true;
						await d.settleNext(o.ok);
					}
					expect(d.live().length).toBeLessThanOrEqual(1);
				}
				while (d.pending.length) await d.settleNext(true);
				const live = d.live();
				if (shown === null) expect(live).toEqual([]);
				else if (failedSinceShown) {
					expect(live.length).toBeLessThanOrEqual(1);
					if (live.length) expect(live[0].container).toBe(shown);
				} else expect(live.map((p) => p.container)).toEqual([shown]);
				anim.hide();
				expect(d.live()).toEqual([]);
			}),
			{ numRuns: 200 }
		);
	});
});
