/**
 * One read at a time for a page that reads per show and per Try
 * again: each read supersedes the ones still out, and only the latest
 * read's answer is shown, so an older one landing late never replaces
 * it.
 *
 * `show` is called with null as a read starts or the reads stop, and
 * with the latest read's answer when it lands. A read must not reject
 * (the watch-order loader answers every failure with data of its own).
 */
export interface LatestRead<T> {
	/** Start a read, superseding any still out; the shown value is null until it answers. */
	start(read: () => Promise<T>): void;
	/** Supersede any read still out without starting another; the shown value is null. */
	stop(): void;
}

export function latestRead<T>(show: (value: T | null) => void): LatestRead<T> {
	return {
		start(read) {
			void read().then(show);
		},
		stop() {}
	};
}
