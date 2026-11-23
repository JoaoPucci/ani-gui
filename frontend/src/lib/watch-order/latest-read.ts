/**
 * One read at a time: each read supersedes the ones still out, and
 * only the latest read's answer is shown, so an older one landing late
 * never replaces it. The watch-order pages read through it: the play
 * page per show, and the franchise page per show and per Try again.
 *
 * Each read is handed a signal, aborted when a later read supersedes
 * it or the reads stop: a franchise read holds its response open while
 * the backend walks the franchise, on the API lane every page shares
 * (`$lib/backend/lanes`), so a read nobody will show must not hold a
 * connection. A page stops its reads when it goes.
 *
 * `show` is called with null as a read starts or the reads stop, and
 * with the latest read's answer when it lands. A read must not reject:
 * callers map a failure to a value of their own (the watch-order loader
 * to its own data).
 */
export interface LatestRead<T> {
	/** Start a read, superseding (and cancelling) any still out; the shown value is null until it answers. */
	start(read: (signal: AbortSignal) => Promise<T>): void;
	/**
	 * Start a read that refreshes what is shown: it supersedes (and
	 * cancels) any read still out, as `start` does, but the shown value
	 * stays until it answers, so the page does not blank meanwhile.
	 */
	refresh(read: (signal: AbortSignal) => Promise<T>): void;
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
	const begin = (read: (signal: AbortSignal) => Promise<T>, blank: boolean) => {
		cancel();
		const mine = ++latest;
		const visit = new AbortController();
		current = visit;
		if (blank) show(null);
		void read(visit.signal).then((value) => {
			if (mine === latest) show(value);
		});
	};
	return {
		start: (read) => begin(read, true),
		refresh: (read) => begin(read, false),
		stop() {
			cancel();
			latest++;
			show(null);
		}
	};
}
