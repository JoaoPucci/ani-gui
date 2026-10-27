// A tracker write the user made, held until the tracker will take it.
//
// AniList limits how many requests this machine may send a minute; the
// backend keeps that budget and, when a write would spend past it or
// AniList is cooling down, refuses it at once with HTTP 429 and the
// seconds until it may go (`retry_after_secs`), sending nothing. A write
// the user made — a list-editor save or removal, a watched episode's
// progress — is not dropped on that answer: it waits the advertised
// time, says so, and goes again, a bounded number of times. The last
// refusal, or any other failure, is the caller's to report.

/** Attempts a refused write gets in all, the first included. */
export const MAX_SAVE_ATTEMPTS = 4;

/** The wait assumed when a 429 names none: AniList's one-minute timeout. */
export const DEFAULT_WAIT_S = 60;

/** The longest a write is held for one refusal. */
export const MAX_WAIT_S = 120;

export { ALL_FIELDS, keepFields, keepWrite, type HeldWrite, type WriteField } from './write-fields';

/** A held write a newer write for the same show replaced while it
 *  waited: dropped, neither sent nor reported. */
export class SupersededError extends Error {
	constructor() {
		super('superseded by a newer write');
	}
}

/**
 * The seconds a refused write is asked to wait, or `null` when `error`
 * is not a rate limit. A 429 whose body carries no usable
 * `retry_after_secs` waits {@link DEFAULT_WAIT_S}, and none waits longer
 * than {@link MAX_WAIT_S}. Read by shape — an
 * `AccountApiError` carries `status` and the raw body in `detail` — so
 * a mocked api module cannot break it.
 */
export function busyWaitSecs(error: unknown): number | null {
	if (!(error instanceof Error)) return null;
	const { status, detail } = error as Error & { status?: unknown; detail?: unknown };
	if (status !== 429) return null;
	const secs = advertisedSecs(typeof detail === 'string' ? detail : '') ?? DEFAULT_WAIT_S;
	return Math.min(secs, MAX_WAIT_S);
}

function advertisedSecs(body: string): number | null {
	try {
		const secs: unknown = JSON.parse(body)?.retry_after_secs;
		return typeof secs === 'number' && Number.isFinite(secs) && secs >= 0 ? Math.ceil(secs) : null;
	} catch {
		return null;
	}
}

export interface SaveWaitDeps {
	/** Tell the user the write waits `seconds` before it goes again. */
	notify: (seconds: number) => void;
	/** Resolve after `ms`. */
	wait: (ms: number) => Promise<void>;
	/** Attempts in all; {@link MAX_SAVE_ATTEMPTS} when omitted. */
	maxAttempts?: number;
	/** Whether the write is still the one to send, asked after each wait. */
	stillWanted?: () => boolean;
}

/**
 * Run `attempt`, and while it is refused as a rate limit, notify, wait
 * the advertised time and run it again — up to the attempt cap. Returns
 * the first answer that lands; rethrows the last refusal, or the first
 * failure that is not a rate limit, as it came. A write `stillWanted`
 * disowns after a wait ends in {@link SupersededError}, unsent.
 */
export async function saveWhenAllowed<T>(
	attempt: () => Promise<T>,
	deps: SaveWaitDeps
): Promise<T> {
	const max = deps.maxAttempts ?? MAX_SAVE_ATTEMPTS;
	for (let tried = 1; ; tried += 1) {
		try {
			return await attempt();
		} catch (e) {
			const secs = busyWaitSecs(e);
			if (secs === null || tried >= max) throw e;
			deps.notify(secs);
			await deps.wait(secs * 1000);
			if (deps.stillWanted && !deps.stillWanted()) throw new SupersededError();
		}
	}
}

/**
 * Wait `ms`, looking every `checkEvery` ms whether the write is still
 * `stillWanted`, and end the wait at the first look that finds it is
 * not: a write dropped while it waits — its account changed, or newer
 * writes took every field — is let go of then, not when the wait is up.
 */
export function waitWhileWanted(
	ms: number,
	deps: {
		after: (ms: number, tick: () => void) => () => void;
		/** The clock the wait is measured on, in milliseconds. */
		now: () => number;
		stillWanted: () => boolean;
		checkEvery?: number;
	}
): Promise<void> {
	const step = deps.checkEvery ?? 1000;
	// Counted in the timer's own steps, not read from the clock: a clock
	// that jumps cannot make the wait longer than it was told.
	let left = Math.max(0, ms);
	return new Promise((resolve) => {
		const look = () => {
			if (left <= 0 || !deps.stillWanted()) {
				resolve();
				return;
			}
			const next = Math.min(step, left);
			left -= next;
			deps.after(next, look);
		};
		const first = Math.min(step, left);
		left -= first;
		deps.after(first, look);
	});
}
