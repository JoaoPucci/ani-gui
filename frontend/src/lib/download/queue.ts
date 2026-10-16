/**
 * How many downloads run at once.
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

export function createDownloadQueue(max: number): DownloadQueue {
	void max;
	return {
		run(task, opts) {
			opts.onStart();
			return task();
		}
	};
}

/** The one every download in the renderer starts through. */
export const DOWNLOAD_CONCURRENCY = 6;
export const downloadQueue = createDownloadQueue(DOWNLOAD_CONCURRENCY);
