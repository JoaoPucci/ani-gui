// A tracker write the user made, held until the tracker will take it.
// Not wired yet: every write runs once, as before.

/** Attempts a refused write gets in all, the first included. */
export const MAX_SAVE_ATTEMPTS = 4;

/** The wait assumed when a 429 names none: AniList's one-minute timeout. */
export const DEFAULT_WAIT_S = 60;

/** The seconds a refused write is asked to wait, or `null` when `error`
 *  is not a rate limit. */
export function busyWaitSecs(error: unknown): number | null {
	void error;
	return null;
}

export interface SaveWaitDeps {
	/** Tell the user the write waits `seconds` before it goes again. */
	notify: (seconds: number) => void;
	/** Resolve after `ms`. */
	wait: (ms: number) => Promise<void>;
	/** Attempts in all; {@link MAX_SAVE_ATTEMPTS} when omitted. */
	maxAttempts?: number;
}

/** Run `attempt` once. */
export async function saveWhenAllowed<T>(
	attempt: () => Promise<T>,
	deps: SaveWaitDeps
): Promise<T> {
	void deps;
	return attempt();
}

/** A real timer, wrapped in an arrow: `setTimeout` called off an object
 *  is an "Illegal invocation" in Chromium. */
export const sleep = (ms: number): Promise<void> =>
	new Promise((resolve) => {
		setTimeout(() => resolve(), ms);
	});
