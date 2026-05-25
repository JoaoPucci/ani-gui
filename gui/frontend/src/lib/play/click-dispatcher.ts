/**
 * Single-click vs double-click resolver for the custom player.
 *
 * The browser's native `dblclick` event still emits two `click`
 * events before it, and `HTMLMediaElement.play()` requires
 * transient user activation that a deferred `setTimeout` callback
 * would have lost. So instead of *delaying* the single-click action
 * to know whether a double is coming, the dispatcher fires the
 * single action immediately — synchronously inside the original
 * click gesture — and gives the caller a hook to *undo* it when a
 * second click promotes the event to a double.
 *
 * For play/pause the undo is just calling `togglePlay` a second
 * time (it's its own inverse), so a double-click nets out to a
 * single fullscreen toggle with no perceived play/pause flicker.
 *
 * Matches the pattern used by YouTube and VLC: synchronous play /
 * pause on click, undo + fullscreen on a fast second click.
 *
 * The helper is framework-agnostic — the caller wires its own
 * click-event source (a `<video>` element, a div, whatever) to
 * `click()` and supplies `onSingle` / optional `onSingleUndo` /
 * `onDouble` callbacks.
 */

/** Default upgrade window in milliseconds. A second click that
 *  arrives inside this window from the first click is promoted to
 *  a double; outside it, the previous single is "committed" and
 *  the new click starts a fresh single. 180 ms is the lowest value
 *  that still catches a deliberate double on mouse + trackpad
 *  without making rapid sequential singles feel paired. */
export const CLICK_DOUBLE_THRESHOLD_MS = 180;

export interface ClickDispatcher {
	/** Feed each native click event into the dispatcher. */
	click(): void;
	/** Cancel any pending upgrade window. Idempotent — safe to call
	 *  from a component-teardown hook even when nothing is pending. */
	dispose(): void;
}

export interface ClickDispatcherOptions {
	/** Fired synchronously on the first click. Runs inside the
	 *  original click gesture so any browser API requiring user
	 *  activation (e.g. `HTMLMediaElement.play()`) works. */
	onSingle: () => void;
	/** Optional: invoked when a second click promotes the event to
	 *  a double. Should reverse the side-effect of `onSingle` so a
	 *  double-click nets out cleanly. When omitted, the single
	 *  side-effect is left in place. */
	onSingleUndo?: () => void;
	/** Fired when a second click lands inside the upgrade window,
	 *  after `onSingleUndo` (if any) has run. */
	onDouble: () => void;
	/** Override the default upgrade window. Useful for tests or for
	 *  tuning per-host (touch devices want a longer window). */
	thresholdMs?: number;
}

export function createClickDispatcher(opts: ClickDispatcherOptions): ClickDispatcher {
	const threshold = opts.thresholdMs ?? CLICK_DOUBLE_THRESHOLD_MS;
	let timer: ReturnType<typeof setTimeout> | null = null;

	function clearTimer() {
		if (timer !== null) {
			clearTimeout(timer);
			timer = null;
		}
	}

	return {
		click() {
			if (timer !== null) {
				// Second click landed inside the window — undo the
				// committed single and apply the double.
				clearTimer();
				opts.onSingleUndo?.();
				opts.onDouble();
				return;
			}
			// First click — commit the single now (preserving user
			// activation) and arm the upgrade window.
			opts.onSingle();
			timer = setTimeout(() => {
				timer = null;
			}, threshold);
		},
		dispose() {
			clearTimer();
		}
	};
}
