/**
 * Open/auto-close gate the Continue Watching row uses to decide
 * whether out:scale + animate:flip should play a real transition
 * or short-circuit to duration 0.
 *
 * Two non-obvious bits this helper exists to test:
 *
 *  1. `dedupeHistoryByKitsuId` collapses sibling rows as their
 *     Kitsu matches resolve on home page mount. Without a gate,
 *     every dedupe-driven removal fires out:scale and every
 *     survivor fires animate:flip — the resize-flicker users see
 *     on Home → Detail → back. The gate stays closed during
 *     load; the factories see `isOn() === false` and skip the
 *     animation.
 *
 *  2. Svelte 5 batches the `history = ...` and `deleteBusy =
 *     false` assignments in `confirmDelete`'s finally block.
 *     By the time Svelte runs the out-transition factory, both
 *     have been applied — gating on `deleteBusy` reads false
 *     and skips the animation the user just triggered. The gate
 *     auto-closes on a timer that fires AFTER Svelte's
 *     synchronous batch processes, so the factory still reads
 *     `isOn() === true` at fire time.
 *
 * `holdMs` defaults to 350ms — the longest transition this gate
 * guards is the 280ms `animate:flip` + 70ms margin. Tune via the
 * factory arg if the transition durations change.
 */
export interface AnimationGate {
	/** Mark the gate open; auto-closes after `holdMs`. Idempotent —
	 *  calling open() again resets the timer. */
	open(): void;
	/** Read at transition fire time. */
	isOn(): boolean;
}

export interface AnimationGateDeps {
	setTimeout: (cb: () => void, ms: number) => unknown;
	clearTimeout: (handle: unknown) => void;
}

const defaultDeps: AnimationGateDeps = {
	setTimeout: (cb, ms) => setTimeout(cb, ms),
	clearTimeout: (h) => clearTimeout(h as Parameters<typeof clearTimeout>[0])
};

export function createAnimationGate(
	holdMs = 350,
	deps: AnimationGateDeps = defaultDeps
): AnimationGate {
	let active = false;
	let timer: unknown = null;
	return {
		open() {
			active = true;
			if (timer !== null) deps.clearTimeout(timer);
			timer = deps.setTimeout(() => {
				active = false;
				timer = null;
			}, holdMs);
		},
		isOn() {
			return active;
		}
	};
}
