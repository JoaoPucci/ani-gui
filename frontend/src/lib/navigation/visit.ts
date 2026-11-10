/**
 * One visit to a page, and the reads it started. Not wired yet: the
 * visit never ends, and every read is passed on as it settles.
 */

/** One visit to a page. */
export interface Visit {
	/** The signal every read of the visit takes. */
	readonly signal: AbortSignal;
	/** The visit has ended. */
	readonly ended: boolean;
	/** The viewer has left. */
	end(): void;
	/** `read`, as it settles. */
	keep<T>(read: Promise<T>): Promise<T>;
}

export function beginVisit(): Visit {
	const { signal } = new AbortController();
	return { signal, ended: false, end: () => {}, keep: (read) => read };
}

/** Whether `e` is what a read rejects with once its signal aborted. */
export function readEnded(e: unknown): boolean {
	void e;
	return false;
}
