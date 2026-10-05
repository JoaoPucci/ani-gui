/**
 * What belongs to the stream a play page has attached: its engine,
 * its listeners, its sidecar tracks. Each registers its retirement
 * here, and the page flushes the scope when the next stream attaches
 * and when the page leaves, so nothing a stream armed outlives it.
 */

export interface SourceScope {
	/** Register a cleanup for the stream attached now. */
	add(fn: () => void): void;
	/** Run and clear every cleanup registered so far. One registered
	 *  while the flush runs belongs to the next stream. */
	flush(): void;
}

export function createSourceScope(): SourceScope {
	let cleanups: (() => void)[] = [];
	return {
		add(fn) {
			cleanups.push(fn);
		},
		flush() {
			const fns = cleanups;
			cleanups = [];
			for (const fn of fns) fn();
		}
	};
}
