// What moved during a scenario.
//
// happy-dom computes no styles, so CSS animations and transitions do
// not exist here; the Electron e2e (`electron/e2e/reduced-motion.spec.ts`)
// asserts on those in a real Chromium. What this tier can see is the
// motion script starts: Svelte's transitions and FLIP, which run
// through `element.animate()`; scrolls asked to glide; and the
// loader's Lottie (see `stubs/lottie-web.ts`). Motion a page writes
// into an inline style, the hero parallax, a scenario reads off the
// element after `scrollWindowTo()`.
//
// A Svelte transition with no duration and no delay starts no
// animation at all. One with either starts a zero-length placeholder
// for the delay and then the real one, so "moved" here is an
// `animate()` call with any time in it.
//
// Neither kind of motion is there the moment the DOM changes. Under
// the `element.animate()` stub (./stubs/element-animate.ts) a
// transition without a delay starts its timed animation a task after
// the update that inserted or removed the element, and a scroll can wait a
// frame for layout. So a scenario never reads `animations` or `glides`
// right after its DOM witness, nor after a fixed sleep: it waits for
// `settled()`, or for the scroll in `scrolls`, and reads them then.
// Under load a sleep is shorter than it looks, and an assertion that
// nothing moved passes just as well before the motion would have
// started as after.

import { animationsInFlight } from './stubs/element-animate';
import { lottieLog } from './stubs/lottie-web';

export interface Motion {
	/** `animate()` calls that had time in them, as `<element> <ms>ms`. */
	readonly animations: string[];
	/** Scrolls asked to glide, as `<method> <element>`. */
	readonly glides: string[];
	/** Every scroll asked of an element, gliding or not, as
	 *  `<method> <element>`: the witness that a scroll was asked at
	 *  all. A scroll of the window is not recorded. */
	readonly scrolls: string[];
	/** True when no animation is running: every transition the DOM's
	 *  changes so far started has started its timed part, and that
	 *  part has finished. Read it once the scenario has seen the DOM
	 *  change: before that update ran nothing has started, and it is
	 *  trivially true. */
	settled(): boolean;
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
	const scrolls: string[] = [];
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
			scrolls.push(`${method} ${describeElement(this)}`);
			if (typeof options === 'object' && options?.behavior === 'smooth') {
				glides.push(`${method} ${describeElement(this)}`);
			}
			return saved[method]?.apply(this, args);
		};
	}

	return {
		animations,
		glides,
		scrolls,
		settled: () => animationsInFlight() === 0,
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
 *
 * Resolves once the change has been taken in. That listener was
 * registered when the module loaded, outside any scenario, so the
 * update it schedules belongs to no scenario either; a page mounted
 * before that update ran would have its first requests counted as
 * nobody's by the request guard in ./setup.ts.
 */
export async function setReducedMotion(reduced: boolean): Promise<void> {
	(window as unknown as HappyDomWindow).happyDOM.settings.device.prefersReducedMotion = reduced
		? 'reduce'
		: 'no-preference';
	window.dispatchEvent(new Event('resize'));
	await new Promise((r) => setTimeout(r, 0));
}

/**
 * Scrolls the window to `y` and sends the `scroll` a real scroll
 * would: happy-dom moves `scrollY` on `scrollTo()` but fires no event.
 * Resolves once the page has re-rendered. `restoreWindowScroll()`
 * scrolls back to the top.
 */
export async function scrollWindowTo(y: number): Promise<void> {
	window.scrollTo(0, y);
	window.dispatchEvent(new Event('scroll'));
	await new Promise((r) => setTimeout(r, 20));
}

/** Puts the window back at the top for the next scenario. */
export function restoreWindowScroll(): void {
	window.scrollTo(0, 0);
}
