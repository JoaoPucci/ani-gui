/**
 * The show page's strip reads on one visit: the visit's walk, and the
 * reads of its rows again that a history link asks for
 * ($lib/history/link-deps `onHistoryLinked`). The page hands in what
 * it draws (`current`), how to read the rows again against it
 * (`reread`, `$lib/watch-order/loader` `rereadRows`) and how to draw
 * an answer (`show`).
 */
export interface StripReads<T> {
	/** The visit's walk: its answer, or null for nothing to draw. */
	walk(answer: Promise<T | null>): void;
	/** A history link answered. */
	linked(): void;
}

export interface StripReadDeps<T> {
	current: () => T | null;
	reread: (shown: T) => Promise<T | null>;
	show: (data: T) => void;
}

export function stripReads<T>(deps: StripReadDeps<T>): StripReads<T> {
	return {
		walk(answer) {
			void answer.then(
				(d) => {
					if (d !== null) deps.show(d);
				},
				() => {}
			);
		},
		linked() {
			const shown = deps.current();
			if (shown === null) return;
			void deps.reread(shown).then(
				(w) => {
					if (w !== null && deps.current() === shown) deps.show(w);
				},
				() => {}
			);
		}
	};
}
