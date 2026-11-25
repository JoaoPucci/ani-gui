/**
 * The player's load tip: a tip shown while a stream loads, before
 * playback starts. Pure: fed the start veil and what blocks the tip as
 * they change, it says what the tip shows and whether it was seen.
 * Rendering: every input is the player's screen.
 *
 * - It shows only once a start has lasted SHOW_AFTER_MS, past the
 *   loader's own beat, so a load that lands sooner never flashes it,
 *   and the tip waits, pending, for a later, slower one.
 * - It never starts in a stall (that is mid-playback), in fullscreen,
 *   over the error panel or over one of the app's dialogs. Any of those
 *   while it is up takes it away, still pending.
 * - When the first frame comes it stays, folded to a small plate in the
 *   frame's corner, until it is closed. A stall then leaves it be.
 * - Closing it (Got it, Escape) is the one way it is seen.
 */
export const SHOW_AFTER_MS = 1000;

export type LoadTipPhase = 'idle' | 'waiting' | 'loading' | 'playing' | 'gone';

export interface LoadTipState {
	phase: LoadTipPhase;
	/** When the start being waited out began (ms). */
	startedAt: number | null;
	/** Closed by the person: seen. */
	seen: boolean;
}

export interface LoadTipInput {
	/** The start veil: `start` from a stream attaching to its first
	 *  frame, `stall` while a playing stream waits for data. */
	veil: 'off' | 'start' | 'stall';
	/** Fullscreen, the error panel, or one of the app's dialogs. */
	blocked: boolean;
	now: number;
}

export const LOAD_TIP_IDLE: LoadTipState = { phase: 'idle', startedAt: null, seen: false };

const gone = (s: LoadTipState): LoadTipState => ({ ...s, phase: 'gone', seen: false });

export function loadTipStep(s: LoadTipState, input: LoadTipInput): LoadTipState {
	const { veil, blocked, now } = input;
	switch (s.phase) {
		case 'gone':
			return s;
		case 'idle':
			return veil === 'start' && !blocked ? { ...s, phase: 'waiting', startedAt: now } : s;
		case 'waiting':
			if (blocked || veil !== 'start') return LOAD_TIP_IDLE;
			return now - (s.startedAt ?? now) >= SHOW_AFTER_MS ? { ...s, phase: 'loading' } : s;
		case 'loading':
			if (blocked) return gone(s);
			return veil === 'start' ? s : { ...s, phase: 'playing' };
		case 'playing':
			return blocked ? gone(s) : s;
	}
}

/** Got it or Escape: gone, and seen. */
export function loadTipClose(s: LoadTipState): LoadTipState {
	return s.phase === 'loading' || s.phase === 'playing' ? { ...s, phase: 'gone', seen: true } : s;
}

/** Whether the tip is on screen. */
export const loadTipUp = (s: LoadTipState): boolean =>
	s.phase === 'loading' || s.phase === 'playing';
