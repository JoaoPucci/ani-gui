/**
 * A short sequence of steps (at most three, the backend's MAX_STEPS),
 * moved with Next, Back and Skip. Rendering: where the balloon stands
 * among the steps on screen.
 *
 * The host scrolls each step's place into view, and leaves out a step
 * whose place is not on the page. A sequence ends as seen by every
 * route that closes it: Done on the last step, Skip, Escape, or a press
 * outside. Back on the first step does nothing.
 */
export const MAX_STEPS = 3;

export interface SeqState {
	index: number;
	total: number;
	done: boolean;
}

export type SeqEvent = 'next' | 'back' | 'skip' | 'close';

export function startSequence(total: number): SeqState {
	if (!Number.isInteger(total) || total < 1 || total > MAX_STEPS) {
		throw new RangeError(`a sequence has 1 to ${MAX_STEPS} steps, not ${total}`);
	}
	return { index: 0, total, done: false };
}

export function stepSequence(s: SeqState, e: SeqEvent): SeqState {
	if (s.done) return s;
	switch (e) {
		case 'next':
			return s.index + 1 >= s.total ? { ...s, done: true } : { ...s, index: s.index + 1 };
		case 'back':
			return { ...s, index: Math.max(0, s.index - 1) };
		case 'skip':
		case 'close':
			return { ...s, done: true };
	}
}

/** The footer's buttons for a step: Skip (quiet, not on a one-step or
 *  the last step), Back (from the second step), and Next, or Done on
 *  the last step. */
export function actionsFor(s: SeqState): { skip: boolean; back: boolean; last: boolean } {
	const last = s.index === s.total - 1;
	return { skip: !last && s.total > 1, back: s.index > 0, last };
}
