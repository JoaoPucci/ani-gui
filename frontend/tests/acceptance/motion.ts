// What moved during a scenario.
//
// happy-dom computes no styles, so CSS animations and transitions do
// not exist here; the Electron e2e (`electron/e2e/reduced-motion.spec.ts`)
// asserts on those in a real Chromium. What this tier can see is the
// motion script starts: Svelte's transitions and FLIP, which run
// through `element.animate()`; scrolls asked to glide; and the
// loader's Lottie (see `stubs/lottie-web.ts`).
//
// A Svelte transition with no duration and no delay starts no
// animation at all. One with either starts a zero-length placeholder
// for the delay and then the real one, so "moved" here is an
// `animate()` call with any time in it.

import { lottieLog } from './stubs/lottie-web';

export interface Motion {
	/** `animate()` calls that had time in them, as `<element> <ms>ms`. */
	readonly animations: string[];
	/** Scrolls asked to glide, as `<method> <element>`. */
	readonly glides: string[];
	/** Everything the Lottie stub was asked since recording began. */
	lottie(): string[];
	/** Puts the patched methods back. */
	stop(): void;
}

function describeElement(el: Element): string {
	const cls =
		typeof el.className === 'string' && el.className
			? `.${el.className.split(/\s+/).join('.')}`
			: '';
	return `${el.tagName.toLowerCase()}${cls}`;
}

type Timing = number | { duration?: number; delay?: number } | undefined;

/** Records motion until `stop()`. Install it before the scenario
 *  mounts anything. */
export function recordMotion(): Motion {
	const animations: string[] = [];
	const glides: string[] = [];
	const lottieFrom = lottieLog.length;
	const proto = Element.prototype as unknown as Record<string, (...args: unknown[]) => unknown>;
	const saved = {
		animate: proto.animate,
		scrollBy: proto.scrollBy,
		scrollTo: proto.scrollTo,
		scrollIntoView: proto.scrollIntoView
	};

	proto.animate = function (this: Element, keyframes: unknown, options: unknown) {
		const timing = options as Timing;
		const duration = typeof timing === 'number' ? timing : (timing?.duration ?? 0);
		const delay = typeof timing === 'number' ? 0 : (timing?.delay ?? 0);
		if (duration > 0 || delay > 0)
			animations.push(`${describeElement(this)} ${duration + delay}ms`);
		return saved.animate.call(this, keyframes, options);
	};
	for (const method of ['scrollBy', 'scrollTo', 'scrollIntoView'] as const) {
		proto[method] = function (this: Element, ...args: unknown[]) {
			const options = args[0] as { behavior?: string } | undefined;
			if (typeof options === 'object' && options?.behavior === 'smooth') {
				glides.push(`${method} ${describeElement(this)}`);
			}
			return saved[method]?.apply(this, args);
		};
	}

	return {
		animations,
		glides,
		lottie: () => lottieLog.slice(lottieFrom),
		stop() {
			Object.assign(proto, saved);
		}
	};
}

interface HappyDomWindow {
	happyDOM: { settings: { device: { prefersReducedMotion: string } } };
}

/**
 * Sets the system's motion preference, as the user flipping the
 * setting would. happy-dom tells a media query list's `change`
 * listeners on the next `resize`, and the app's one reading of the
 * preference (`$lib/motion/reduced-motion.svelte`) is such a listener,
 * so this also exercises the app following a change while it is open.
 */
export function setReducedMotion(reduced: boolean): void {
	(window as unknown as HappyDomWindow).happyDOM.settings.device.prefersReducedMotion = reduced
		? 'reduce'
		: 'no-preference';
	window.dispatchEvent(new Event('resize'));
}
