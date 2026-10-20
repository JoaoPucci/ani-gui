/**
 * How many downloads run at once.
 *
 * A download reports its progress over a stream of its own, open for
 * the whole download, and the backend starts the download only when
 * that stream arrives. Downloads have a lane of their own
 * (`$lib/backend/lanes`), so they take no connection from the player
 * or the API — but Chromium opens at most six connections to that
 * lane, and a seventh download's stream would wait inside Chromium for
 * one of them while its row read as started. So the renderer starts
 * at most six at once and holds the rest here, where the dock can say
 * they are queued and a cancel can drop them before they start.
 *
 *   - a download given a slot starts in the same turn it was asked;
 *   - the rest start in order as each running one ends, finished or
 *     failed;
 *   - a waiting download whose signal aborts leaves the queue and
 *     never starts.
 */

export interface DownloadRunOptions {
	/** Aborts the download, whether it is running or still waiting. */
	signal: AbortSignal;
	/** Called when the download is given a slot and starts. */
	onStart: () => void;
}

export interface DownloadQueue {
	run<T>(task: () => Promise<T>, opts: DownloadRunOptions): Promise<T>;
}

// i18n-ignore: a DOMException's message and name, never shown to the user
const abortError = () => new DOMException('The download was cancelled.', 'AbortError');

export function createDownloadQueue(max: number): DownloadQueue {
	let running = 0;
	const waiting: Array<() => void> = [];

	const next = () => {
		while (running < max && waiting.length > 0) waiting.shift()!();
	};

	const run = async <T>(task: () => Promise<T>, opts: DownloadRunOptions): Promise<T> => {
		running += 1;
		try {
			opts.onStart();
			return await task();
		} finally {
			running -= 1;
			next();
		}
	};

	return {
		run(task, opts) {
			if (opts.signal.aborted) return Promise.reject(abortError());
			if (running < max) return run(task, opts);
			return new Promise((resolve, reject) => {
				const start = () => {
					opts.signal.removeEventListener('abort', onAbort);
					run(task, opts).then(resolve, reject);
				};
				const onAbort = () => {
					const at = waiting.indexOf(start);
					if (at >= 0) waiting.splice(at, 1);
					reject(abortError());
				};
				opts.signal.addEventListener('abort', onAbort, { once: true });
				waiting.push(start);
			});
		}
	};
}

/** As many as Chromium opens connections to the download lane. */
export const DOWNLOAD_CONCURRENCY = 6;

/** The one every download in the renderer starts through. */
export const downloadQueue = createDownloadQueue(DOWNLOAD_CONCURRENCY);
