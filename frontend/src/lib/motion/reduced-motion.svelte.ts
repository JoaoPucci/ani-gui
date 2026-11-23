/**
 * Whether the user asked for reduced motion — the one place the app
 * reads it.
 *
 * Under reduced motion nothing in the app moves: transitions land at
 * their end state at once, loops hold still, scrolls jump instead of
 * gliding and the Home hero stops rotating. The app's motion is soft
 * already, so there is no gentler fallback to offer; "reduce" means
 * stop.
 *
 * Stylesheets read the preference through the media query directly —
 * `tokens.css` carries the one rule that stops every CSS animation and
 * transition. Script reads it here, through `reducedMotion.current`,
 * which is reactive: an `$effect` or a template that reads it runs
 * again when the system setting changes while the app is open. Code
 * that is not reactive subscribes with `reducedMotion.onChange`.
 *
 * `src/lib/motion/` is the only directory that may ask `matchMedia` for
 * this query, import Svelte's transitions, start a Web Animation or
 * glide a scroll; `tests/lint/reduced-motion.test.ts` holds that line.
 */

export const REDUCED_MOTION_QUERY = '(prefers-reduced-motion: reduce)';

/** The part of a `MediaQueryList` this module reads. */
export interface MotionQuery {
	readonly matches: boolean;
	addEventListener(type: 'change', listener: () => void): void;
	removeEventListener(type: 'change', listener: () => void): void;
}

export interface ReducedMotion {
	/** True when the user asked for reduced motion. Reactive. */
	readonly current: boolean;
	/** Calls `listener` with the new preference each time it changes;
	 *  returns the unsubscribe. */
	onChange(listener: (reduced: boolean) => void): () => void;
}

/** The system's reduced-motion query, or null where there is no
 *  window to ask (a server render, a unit test in node). */
export function systemMotionQuery(
	win: { matchMedia?: (query: string) => MotionQuery } | undefined
): MotionQuery | null {
	if (typeof win?.matchMedia !== 'function') return null;
	return win.matchMedia(REDUCED_MOTION_QUERY);
}

export function createReducedMotion(query: MotionQuery | null): ReducedMotion {
	let reduced = $state(query?.matches ?? false);
	// The listeners are callbacks to call, not state anything renders.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const listeners = new Set<(reduced: boolean) => void>();
	query?.addEventListener('change', () => {
		reduced = query.matches;
		for (const listener of [...listeners]) listener(reduced);
	});
	return {
		get current() {
			return reduced;
		},
		onChange(listener) {
			listeners.add(listener);
			return () => listeners.delete(listener);
		}
	};
}

export const reducedMotion: ReducedMotion = createReducedMotion(
	systemMotionQuery(typeof window === 'undefined' ? undefined : window)
);
