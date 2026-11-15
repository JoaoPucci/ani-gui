/**
 * One read at a time: each read supersedes the ones still out, and
 * only the latest read's answer is shown, so an older one landing late
 * never replaces it. The watch-order pages read through it: the play
 * page per show, and the franchise page per show and per Try again.
 *
 * Each read is handed a signal, aborted when a later read supersedes
 * it or the reads stop: a franchise read holds its response open while
 * the backend walks the franchise, on the franchise lane every page's
 * franchise reads share (`$lib/backend/lanes`), so a read nobody will
 * show must not hold a connection. A page stops its reads when it goes.
 *
 * `show` is called with null as a read starts or the reads stop, and
 * with the latest read's answer when it lands. A read must not reject:
 * callers map a failure to a value of their own (the watch-order loader
 * to its own data).
 */
export interface LatestRead<T> {
	/** Start a read, superseding (and cancelling) any still out; the shown value is null until it answers. */
	start(read: (signal: AbortSignal) => Promise<T>): void;
	/** Supersede (and cancel) any read still out without starting another; the shown value is null. */
	stop(): void;
}

export function latestRead<T>(show: (value: T | null) => void): LatestRead<T> {
	let latest = 0;
	let current: AbortController | null = null;
	const cancel = () => {
		current?.abort();
		current = null;
	};
	return {
		start(read) {
			cancel();
			const mine = ++latest;
			const visit = new AbortController();
			current = visit;
			show(null);
			void read(visit.signal).then((value) => {
				if (mine === latest) show(value);
			});
		},
		stop() {
			cancel();
			latest++;
			show(null);
		}
	};
}
