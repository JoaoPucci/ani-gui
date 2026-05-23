import { describe, expect, it } from 'vitest';
import { decidePlayerKeyAction, PLAYER_SEEK_STEP_SECONDS, type PlayerKeyContext } from './keyboard';

const baseCtx: PlayerKeyContext = {
	key: '',
	inField: false,
	inButton: false,
	modifier: false
};

describe('decidePlayerKeyAction', () => {
	it('maps Space (modern KeyboardEvent.key) to togglePlay', () => {
		expect(decidePlayerKeyAction({ ...baseCtx, key: ' ' })).toEqual({ kind: 'togglePlay' });
	});

	it('maps Spacebar (legacy KeyboardEvent.key) to togglePlay', () => {
		// Some older browsers / IE-compat shims surface space as
		// 'Spacebar' instead of ' '. Cover both so the shortcut works
		// regardless of the runtime's key normalization.
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'Spacebar' })).toEqual({ kind: 'togglePlay' });
	});

	it('maps ArrowLeft / ArrowRight to seek by ±PLAYER_SEEK_STEP_SECONDS', () => {
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'ArrowLeft' })).toEqual({
			kind: 'seek',
			deltaSeconds: -PLAYER_SEEK_STEP_SECONDS
		});
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'ArrowRight' })).toEqual({
			kind: 'seek',
			deltaSeconds: PLAYER_SEEK_STEP_SECONDS
		});
	});

	it('maps n / N to next, p / P to prev, f / F to fullscreen', () => {
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'n' })).toEqual({ kind: 'next' });
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'N' })).toEqual({ kind: 'next' });
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'p' })).toEqual({ kind: 'prev' });
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'P' })).toEqual({ kind: 'prev' });
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'f' })).toEqual({ kind: 'fullscreen' });
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'F' })).toEqual({ kind: 'fullscreen' });
	});

	it('returns null for unmapped keys', () => {
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'a' })).toBeNull();
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'Enter' })).toBeNull();
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'Escape' })).toBeNull();
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'ArrowUp' })).toBeNull();
	});

	it('returns null when focus is in a text-entry field', () => {
		// Typing a letter must reach the input, and pressing space in
		// a search box must insert a space — not toggle the player.
		expect(decidePlayerKeyAction({ ...baseCtx, key: ' ', inField: true })).toBeNull();
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'ArrowLeft', inField: true })).toBeNull();
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'n', inField: true })).toBeNull();
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'f', inField: true })).toBeNull();
	});

	it('returns null when a modifier key is held', () => {
		// Cmd+ArrowLeft is "go back in history" on Mac; Ctrl+F is
		// browser find; Alt+ArrowLeft is back on most platforms. The
		// player must never shadow those.
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'ArrowLeft', modifier: true })).toBeNull();
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'f', modifier: true })).toBeNull();
		expect(decidePlayerKeyAction({ ...baseCtx, key: ' ', modifier: true })).toBeNull();
	});

	it('returns null on Space when focus is on an interactive activation element', () => {
		// Space is the default activation key for buttons and
		// links — clicking "Watch together" via the keyboard must
		// trigger the button, not toggle the player. Arrow keys are
		// safe to intercept here because buttons don't react to
		// arrows by default.
		expect(decidePlayerKeyAction({ ...baseCtx, key: ' ', inButton: true })).toBeNull();
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'Spacebar', inButton: true })).toBeNull();
	});

	it('still intercepts arrow keys when focus is on an interactive element', () => {
		// Buttons don't react to arrows by default, so seeking via
		// arrows works even when focus happens to be on the play
		// button itself.
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'ArrowLeft', inButton: true })).toEqual({
			kind: 'seek',
			deltaSeconds: -PLAYER_SEEK_STEP_SECONDS
		});
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'ArrowRight', inButton: true })).toEqual({
			kind: 'seek',
			deltaSeconds: PLAYER_SEEK_STEP_SECONDS
		});
	});

	it('still intercepts letter shortcuts when focus is on an interactive element', () => {
		// `n` / `p` / `f` are single-character page shortcuts that
		// don't collide with button activation. The play-page
		// shortcuts predate this fix and weren't gated on focus.
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'n', inButton: true })).toEqual({
			kind: 'next'
		});
		expect(decidePlayerKeyAction({ ...baseCtx, key: 'f', inButton: true })).toEqual({
			kind: 'fullscreen'
		});
	});
});
