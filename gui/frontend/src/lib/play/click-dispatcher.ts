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
 * The helper is framework-agnostic — the caller wires its own
 * click-event source (a `<video>` element, a div, whatever) to
 * `click()` and supplies `onSingle` / optional `onSingleUndo` /
 * `onDouble` callbacks.
 */

/** Default upgrade window in milliseconds. A second click that
 *  arrives inside this window from the first click is promoted to
 *  a double; outside it, the previous single is "committed" and
 *  the new click starts a fresh single. 300 ms is wide enough to
 *  catch most users' deliberate doubles — including slower clickers
 *  and accessibility-tuned OS double-click settings — and because
 *  the dispatcher is synchronous-first the width doesn't add any
 *  perceived latency to single clicks. */
export const CLICK_DOUBLE_THRESHOLD_MS = 300;

/** Default max pointer drift between the two clicks of a double,
 *  in CSS pixels. Beyond this the second click is treated as a
 *  fresh single — two quick clicks in different parts of the video
 *  must not accidentally promote to fullscreen. 30px is wider than
 *  Chromium's touch-tap slop (~8px) so mouse users get a generous
 *  margin without losing the "same hit area" guarantee. */
export const CLICK_DOUBLE_MAX_DISTANCE_PX = 30;

export interface ClickPoint {
	x: number;
	y: number;
}

export interface ClickDispatcher {
	/** Feed each native click event into the dispatcher. The point
	 *  is the event's `clientX` / `clientY` (or any consistent
	 *  coordinate space — only relative distance matters). */
	click(point: ClickPoint): void;
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
	/** Fired when a second click lands inside the upgrade window
	 *  *and* within the distance slop, after `onSingleUndo` (if
	 *  any) has run. */
	onDouble: () => void;
	/** Override the default upgrade window. Useful for tests or for
	 *  tuning per-host (touch devices want a longer window). */
	thresholdMs?: number;
	/** Override the default same-hit-area slop. Useful for tests
	 *  or for tightening when the click target is small. */
	maxDistancePx?: number;
}

export function createClickDispatcher(opts: ClickDispatcherOptions): ClickDispatcher {
	const threshold = opts.thresholdMs ?? CLICK_DOUBLE_THRESHOLD_MS;
	const maxDistance = opts.maxDistancePx ?? CLICK_DOUBLE_MAX_DISTANCE_PX;
	let timer: ReturnType<typeof setTimeout> | null = null;
	let firstPoint: ClickPoint | null = null;

	function clearTimer() {
		if (timer !== null) {
			clearTimeout(timer);
			timer = null;
		}
		firstPoint = null;
	}

	function armWindow(point: ClickPoint) {
		firstPoint = point;
		timer = setTimeout(() => {
			timer = null;
			firstPoint = null;
		}, threshold);
	}

	return {
		click(point: ClickPoint) {
			if (timer !== null && firstPoint !== null) {
				const dx = point.x - firstPoint.x;
				const dy = point.y - firstPoint.y;
				const distance = Math.hypot(dx, dy);
				if (distance <= maxDistance) {
					// Same hit area, inside the window — promote.
					clearTimer();
					opts.onSingleUndo?.();
					opts.onDouble();
					return;
				}
				// Pointer drifted too far for a real double. Treat this
				// click as a fresh single and arm a new window from here.
				clearTimer();
			}
			opts.onSingle();
			armWindow(point);
		},
		dispose() {
			clearTimer();
		}
	};
}
