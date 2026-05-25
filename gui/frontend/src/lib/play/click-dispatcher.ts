/**
 * Single-click vs double-click resolver for the custom player.
 *
 * The browser's native `dblclick` event still emits two `click`
 * events before it, so wiring `togglePlay` to `click` and
 * `toggleFullscreen` to `dblclick` would flip play/pause twice
 * before fullscreen kicks in — flicker the user can see and hear.
 * Instead this helper defers the single-click action by a small
 * window; if a second click arrives within it, the pending single
 * is cancelled and `onDouble` fires immediately.
 *
 * Matches the pattern YouTube and VLC use: ~250 ms of latency on
 * single-click is below the threshold most users notice, and the
 * double-click feedback is clean.
 *
 * The helper is framework-agnostic — the caller wires its own
 * click-event source (a `<video>` element, a div, whatever) to
 * `click()` and supplies `onSingle` / `onDouble` callbacks.
 */

/** Default window in milliseconds during which a second click counts
 *  as the second half of a double-click. The OS-level default is
 *  ~250 ms, but play/pause is the dominant single-click action in a
 *  video player so the perceived lag at that threshold is too long;
 *  180 ms is the lowest value that still reliably catches a deliberate
 *  double-click without making single clicks feel sticky. */
export const CLICK_DOUBLE_THRESHOLD_MS = 180;

export interface ClickDispatcher {
	/** Feed each native click event into the dispatcher. */
	click(): void;
	/** Cancel any pending single-click timer. Idempotent — safe to
	 *  call from a component-teardown hook even when nothing is
	 *  pending. */
	dispose(): void;
}

export interface ClickDispatcherOptions {
	onSingle: () => void;
	onDouble: () => void;
	/** Override the default threshold. Useful for tests or for tuning
	 *  per-host (touch devices want a longer window). */
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
				// Second click landed inside the window — promote to
				// double and drop the pending single.
				clearTimer();
				opts.onDouble();
				return;
			}
			timer = setTimeout(() => {
				timer = null;
				opts.onSingle();
			}, threshold);
		},
		dispose() {
			clearTimer();
		}
	};
}
