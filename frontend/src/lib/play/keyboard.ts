/**
 * Page-level keyboard-shortcut decision for the custom video player.
 *
 * Kept pure so the play page can be a thin adapter: the .svelte
 * handler reads the active element, hands the relevant context to
 * `decidePlayerKeyAction`, and dispatches to the existing
 * `togglePlay` / `seekToFraction` / `onNext` / `onPrev` /
 * `toggleFullscreen` callbacks.
 *
 * Shortcut surface (sighted-user, page-level — the screen-reader
 * slider role on the scrubber stays independent):
 *
 *   Space             → toggle play / pause
 *   ArrowLeft         → seek -5 s
 *   ArrowRight        → seek +5 s
 *   ArrowUp           → volume +5 %
 *   ArrowDown         → volume -5 %
 *   n / N             → next episode
 *   p / P             → previous episode
 *   f / F             → toggle fullscreen
 *
 * Suppression rules (return null, let the browser handle the key):
 *
 *   - Focus is in a text-entry field (`INPUT`, `TEXTAREA`,
 *     `contentEditable`) — typing the letter or pressing space must
 *     reach the field, not the player.
 *   - A modifier (Ctrl/Cmd/Alt) is held — don't shadow browser
 *     shortcuts like Ctrl+F (find) or Alt+ArrowLeft (history back).
 *   - `Space` while focus is on an interactive element (`BUTTON`,
 *     `A`, `SELECT`, `role=button`) — Space is the default
 *     activation key for those, so the player must not steal it.
 *     Arrow keys are safe to intercept here because buttons don't
 *     react to arrows by default.
 *   - The `KeyboardEvent.repeat` auto-repeat firings for the
 *     toggle-style actions (`Space`, `n`, `p`, `f`) — holding the
 *     key would otherwise flicker the state every few ms. Arrow
 *     seeks intentionally still fire on repeat so holding `→`
 *     scrubs forward continuously, matching the YouTube habit.
 */

export type PlayerKeyAction =
	| { kind: 'togglePlay' }
	| { kind: 'seek'; deltaSeconds: number }
	| { kind: 'volume'; delta: number }
	| { kind: 'next' }
	| { kind: 'prev' }
	| { kind: 'fullscreen' };

/** Seconds the arrow keys seek by — matches the previous scrubber-
 *  focused inline handler so muscle memory carries over. */
export const PLAYER_SEEK_STEP_SECONDS = 5;

/** Volume delta the up/down arrows nudge by, on the [0, 1] scale.
 *  5 % matches YouTube and Netflix; the slider's `step="0.01"`
 *  remains the finer-grained path when the user has explicitly
 *  focused the range input. */
export const PLAYER_VOLUME_STEP = 0.05;

export interface PlayerKeyContext {
	/** Pressed key (KeyboardEvent.key). */
	key: string;
	/** True when the event target is an input/textarea/contentEditable
	 *  element — typing must reach the field, not the player. */
	inField: boolean;
	/** True when the event target is an interactive activation
	 *  element (button, anchor, select, role=button). Used only to
	 *  gate `Space` — arrows are safe everywhere. */
	inButton: boolean;
	/** True if any of Ctrl / Cmd / Alt is held. */
	modifier: boolean;
	/** `KeyboardEvent.repeat` — true once the OS starts auto-firing
	 *  `keydown` while the key is held (typically after ~500 ms).
	 *  Toggle-style actions ignore repeats so a long hold doesn't
	 *  flicker the state; arrow seeks let repeats through for
	 *  continuous scrubbing. */
	repeat: boolean;
}

export function decidePlayerKeyAction(ctx: PlayerKeyContext): PlayerKeyAction | null {
	if (ctx.inField) return null;
	if (ctx.modifier) return null;

	switch (ctx.key) {
		case ' ':
		case 'Spacebar':
			// Don't shadow native button-activation Space.
			if (ctx.inButton) return null;
			// Auto-repeat would flicker play/pause on a long hold.
			if (ctx.repeat) return null;
			return { kind: 'togglePlay' };
		case 'ArrowLeft':
			// Repeated seeks while holding the arrow are useful
			// (continuous scrub) — let them through.
			return { kind: 'seek', deltaSeconds: -PLAYER_SEEK_STEP_SECONDS };
		case 'ArrowRight':
			return { kind: 'seek', deltaSeconds: PLAYER_SEEK_STEP_SECONDS };
		case 'ArrowUp':
			// Repeat-friendly like seek — holding `↑` ramps volume up.
			return { kind: 'volume', delta: PLAYER_VOLUME_STEP };
		case 'ArrowDown':
			return { kind: 'volume', delta: -PLAYER_VOLUME_STEP };
		case 'n':
		case 'N':
			if (ctx.repeat) return null;
			return { kind: 'next' };
		case 'p':
		case 'P':
			if (ctx.repeat) return null;
			return { kind: 'prev' };
		case 'f':
		case 'F':
			if (ctx.repeat) return null;
			return { kind: 'fullscreen' };
		default:
			return null;
	}
}
