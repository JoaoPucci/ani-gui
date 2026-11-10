/**
 * One read at a time: each read supersedes the ones still out, and
 * only the latest read's answer is shown, so an older one landing late
 * never replaces it. The play page reads its watch order per show
 * through it.
 *
 * `show` is called with null as a read starts or the reads stop, and
 * with the latest read's answer when it lands. A read must not reject:
 * callers map a failure to a value of their own (the watch-order loader
 * to its own data).
 */
export interface LatestRead<T> {
	/** Start a read, superseding any still out; the shown value is null until it answers. */
	start(read: () => Promise<T>): void;
	/** Supersede any read still out without starting another; the shown value is null. */
	stop(): void;
}

export function latestRead<T>(show: (value: T | null) => void): LatestRead<T> {
	let latest = 0;
	return {
		start(read) {
			const mine = ++latest;
			show(null);
			void read().then((value) => {
				if (mine === latest) show(value);
			});
		},
		stop() {
			latest++;
			show(null);
		}
	};
}
