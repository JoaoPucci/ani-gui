/**
 * Volume-pill reveal state machine.
 *
 * Driven by the page-level keyboard handler: every ArrowUp/Down
 * keypress calls `trigger()`, which flips the visible signal to
 * `true` synchronously (so the pill expands on the same frame as the
 * keystroke) and schedules a single hide `VOLUME_REVEAL_HOLD_MS`
 * later. Rapid retriggers replace the pending timer rather than
 * stacking, so holding the key — which auto-fires `keydown` every
 * ~30 ms — does not let the pill briefly blink off between events.
 *
 * Lives in `$lib/play` rather than inline in `+page.svelte` so the
 * state-machine edges (synchronous reveal, retrigger replaces
 * timer, dispose clears pending) can be unit-tested directly per
 * AGENTS.md §2.
 *
 * The caller owns the `revealed` storage (typically a Svelte `$state`
 * boolean) and passes a setter to the factory; the helper never
 * touches Svelte runtime APIs, which keeps it framework-agnostic
 * and the tests trivial.
 */

/** Milliseconds the pill stays visible after the last trigger.
 *  ~1.2 s matches YouTube's volume HUD dwell time — long enough to
 *  read, short enough not to feel sticky. */
export const VOLUME_REVEAL_HOLD_MS = 1200;

export interface VolumeReveal {
	/** Show the pill now; auto-hide after VOLUME_REVEAL_HOLD_MS unless
	 *  another `trigger()` lands first. Idempotent within the hold
	 *  window — back-to-back triggers refresh the timer but do not
	 *  re-emit the visible=true callback. */
	trigger(): void;
	/** Cancel any pending hide timer and force visible=false. Safe to
	 *  call from a component-teardown hook and idempotent — no-ops
	 *  when already hidden with no pending timer. */
	dispose(): void;
}

/** Build a reveal state machine that notifies `onChange` whenever the
 *  visible flag transitions. The caller is responsible for storing
 *  the boolean and re-rendering — typically by writing into a
 *  Svelte `$state` variable inside the callback. */
export function createVolumeReveal(onChange: (visible: boolean) => void): VolumeReveal {
	let visible = false;
	let timer: ReturnType<typeof setTimeout> | null = null;

	function clearTimer() {
		if (timer !== null) {
			clearTimeout(timer);
			timer = null;
		}
	}

	function setVisible(next: boolean) {
		if (next === visible) return;
		visible = next;
		onChange(next);
	}

	return {
		trigger() {
			setVisible(true);
			clearTimer();
			timer = setTimeout(() => {
				timer = null;
				setVisible(false);
			}, VOLUME_REVEAL_HOLD_MS);
		},
		dispose() {
			clearTimer();
			setVisible(false);
		}
	};
}
