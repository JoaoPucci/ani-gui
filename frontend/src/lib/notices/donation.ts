/**
 * When to ask for a donation: the state machine, pure.
 *
 * The maintainer publishes a number; each install remembers the last
 * one it acted on and asks once each time it sees a higher one. The
 * safeguards:
 *
 * - a new install records the number current when it is installed and
 *   reacts only to later rises (an install that saw no number takes the
 *   first one it sees as that baseline);
 * - nothing in the first {@link GRACE_MS} after the first launch that
 *   kept this state (the first with the center on), and
 *   an ask held back by it is made after the period if the number is
 *   still unseen;
 * - never while a video plays: it waits for a moment with nothing
 *   playing;
 * - shown once: the number is acted on when the ask is shown, so
 *   dismissing it ("Not now") or acting on it changes nothing more.
 *
 * `optOut` is reserved for a permanent "Don't ask again" a later
 * version may offer; nothing sets it today, and a state that has it is
 * never asked. Keeping it in the stored shape now means adding the
 * switch needs no migration.
 *
 * Wired by ./donation-ask: the number comes from the published
 * notices of a verified flags file (./published).
 */

export const GRACE_MS = 14 * 24 * 3600_000;

export const DONATION_STORAGE_KEY = 'ani-gui.donation-ask.v1';

export interface DonationState {
	v: 1;
	/** The first launch that kept this state, in ms. */
	firstLaunchAt: number;
	/** The number current at install, or null until one is seen. */
	baseline: number | null;
	/** The last number the install asked for. */
	acted: number | null;
	/** Reserved: a permanent opt-out. Never set today. */
	optOut: boolean;
}

export interface Launch {
	/** The published number, or null when there is none. */
	n: number | null;
	now: number;
	playing: boolean;
}

export function decide(
	state: DonationState | null,
	at: Launch
): { state: DonationState; ask: boolean } {
	if (state === null) {
		return {
			state: { v: 1, firstLaunchAt: at.now, baseline: at.n, acted: null, optOut: false },
			ask: false
		};
	}
	if (at.n === null) return { state, ask: false };
	if (state.baseline === null) return { state: { ...state, baseline: at.n }, ask: false };
	const last = Math.max(state.baseline, state.acted ?? state.baseline);
	const due =
		!state.optOut && at.n > last && at.now - state.firstLaunchAt >= GRACE_MS && !at.playing;
	return due ? { state: { ...state, acted: at.n }, ask: true } : { state, ask: false };
}

const count = (v: unknown): number | null =>
	typeof v === 'number' && Number.isInteger(v) && v >= 0 ? v : null;

export function serializeState(s: DonationState): string {
	return JSON.stringify(s);
}

/** The stored state, or null when there is none or it is not one. */
export function parseState(raw: string | null): DonationState | null {
	if (!raw) return null;
	let o: unknown;
	try {
		o = JSON.parse(raw);
	} catch {
		return null;
	}
	if (typeof o !== 'object' || o === null) return null;
	const r = o as Record<string, unknown>;
	if (r.v !== 1 || typeof r.firstLaunchAt !== 'number' || !Number.isFinite(r.firstLaunchAt))
		return null;
	return {
		v: 1,
		firstLaunchAt: r.firstLaunchAt,
		baseline: count(r.baseline),
		acted: count(r.acted),
		optOut: r.optOut === true
	};
}
