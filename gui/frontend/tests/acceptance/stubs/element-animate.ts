// happy-dom has no Web Animations API, and Svelte transitions call
// `element.animate()` directly.
//
// This stays invisible until a scenario changes the DOM *after*
// mount. Svelte skips intro transitions on the initial render, so a
// page that renders its strip once never animates; a page that
// splices a tile in later — a special the availability recheck
// found, say — hits `element.animate is not a function` inside the
// transition. The test still passes, because the throw happens in
// Svelte's own scheduling rather than in the assertion, and the run
// exits non-zero with an unhandled error instead.
//
// So this is not about making transitions work. It is about the DOM
// implementation being complete enough that a scenario fails for its
// own reasons.

/** The subset of `Animation` Svelte's transition machinery touches. */
interface FakeAnimation {
	onfinish: (() => void) | null;
	oncancel: (() => void) | null;
	currentTime: number;
	startTime: number;
	playbackRate: number;
	effect: { getComputedTiming: () => { delay: number; duration: number; endTime: number } };
	finished: Promise<void>;
	cancel: () => void;
	finish: () => void;
	play: () => void;
	pause: () => void;
	commitStyles: () => void;
	persist: () => void;
}

export function installElementAnimateStub(): void {
	if (typeof Element === 'undefined') return;
	const proto = Element.prototype as unknown as Record<string, unknown>;
	if (typeof proto.animate === 'function') return;

	proto.animate = function animate(
		_keyframes: unknown,
		options?: number | { duration?: number; delay?: number }
	): FakeAnimation {
		const duration = typeof options === 'number' ? options : (options?.duration ?? 0);
		const delay = typeof options === 'number' ? 0 : (options?.delay ?? 0);
		let done = false;
		const animation: FakeAnimation = {
			onfinish: null,
			oncancel: null,
			currentTime: 0,
			startTime: 0,
			playbackRate: 1,
			effect: {
				getComputedTiming: () => ({ delay, duration, endTime: delay + duration })
			},
			finished: Promise.resolve(),
			cancel() {
				if (done) return;
				done = true;
				animation.oncancel?.();
			},
			finish() {
				if (done) return;
				done = true;
				animation.onfinish?.();
			},
			play() {},
			pause() {},
			commitStyles() {},
			persist() {}
		};
		// Settle on a macrotask rather than instantly: Svelte assigns
		// `onfinish` after `animate()` returns, so finishing inline
		// would call a handler that does not exist yet and leave the
		// element mid-transition forever.
		setTimeout(() => animation.finish(), 0);
		return animation;
	};
}
