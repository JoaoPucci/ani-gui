/**
 * One visit to a page, and the reads it started.
 *
 * A read holds one of the API lane's six connections until it answers
 * (`$lib/backend/lanes`), and Chromium queues every request past the
 * sixth. A read the page started and nobody will read after the viewer
 * left — a show's details, its similar titles, a row's match, the
 * settings — went on holding its connection, and enough of them left
 * the next page's own reads waiting behind them.
 *
 * So each page begins a visit and hands its signal to every read it
 * starts; the page's cleanup ends the visit, which cancels whatever is
 * still out. A page whose component outlives a change of what it shows
 * — one show's page moving to another — begins a visit per show and
 * ends the old one as the next begins.
 *
 * A cancelled read rejects with the abort. Nothing of an ended visit
 * may reach the screen as a failure or as nothing to show: a page
 * passes the reads it answers from through `keep`, which settles while
 * the visit lasts and never after, so neither a late answer nor the
 * abort itself is shown about a visit the viewer left.
 */

/** One visit to a page. */
export interface Visit {
	/** The signal every read of the visit takes. */
	readonly signal: AbortSignal;
	/** The visit has ended. */
	readonly ended: boolean;
	/** The viewer has left: every read of the visit still out is
	 *  cancelled. Ending twice changes nothing. */
	end(): void;
	/** `read`, settled as it settles while the visit lasts; once the
	 *  visit has ended, a promise that never settles — neither the
	 *  answer nor the failure (the abort among them) is passed on. */
	keep<T>(read: Promise<T>): Promise<T>;
}

export function beginVisit(): Visit {
	const controller = new AbortController();
	const { signal } = controller;
	return {
		signal,
		get ended() {
			return signal.aborted;
		},
		end: () => controller.abort(),
		keep: <T>(read: Promise<T>) =>
			new Promise<T>((resolve, reject) => {
				read.then(
					(value) => {
						if (!signal.aborted) resolve(value);
					},
					(error: unknown) => {
						if (!signal.aborted) reject(error);
					}
				);
			})
	};
}

/** Whether `e` is what a read rejects with once its signal aborted —
 *  the visit ending — rather than a failure of the backend or the
 *  network, which says something about what was asked. */
export function readEnded(e: unknown): boolean {
	return e instanceof DOMException && e.name === 'AbortError';
}
