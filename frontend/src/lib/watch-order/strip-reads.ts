/**
 * The show page's strip reads on one visit: the visit's walk, and the
 * reads of its rows again that a history link asks for
 * ($lib/history/link-deps `onHistoryLinked`). The page hands in what
 * it draws (`current`), how to read the rows again against it
 * (`reread`, `$lib/watch-order/loader` `rereadRows`) and how to draw
 * an answer (`show`).
 *
 * A link answered while the walk is out is held until the walk lands,
 * whose rows may have been read before the link, and then reads the
 * rows again. Of the re-reads, only the latest started is drawn, so
 * two links in a row end on the rows the second read, whichever lands
 * first; a re-read of the visit before is never drawn.
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
	// Each walk starts a generation, and only its own answer and the
	// re-reads started within it are drawn.
	let generation = 0;
	// The latest re-read started: only its answer is drawn, so two links
	// in a row end on the rows the second read.
	let latest = 0;
	let walking = false;
	let linkedWhileWalking = false;
	const reread = () => {
		const shown = deps.current();
		if (shown === null) return;
		const mine = ++latest;
		const gen = generation;
		void deps.reread(shown).then(
			(w) => {
				if (w !== null && mine === latest && gen === generation) deps.show(w);
			},
			() => {}
		);
	};
	// The walk has settled: a link answered while it was out may have
	// come after its rows were read, so they are read again.
	const walked = (gen: number) => {
		if (gen !== generation) return false;
		walking = false;
		if (linkedWhileWalking) {
			linkedWhileWalking = false;
			reread();
		}
		return true;
	};
	return {
		walk(answer) {
			const gen = ++generation;
			walking = true;
			linkedWhileWalking = false;
			void answer.then(
				(d) => {
					if (gen !== generation) return;
					if (d !== null) deps.show(d);
					walked(gen);
				},
				() => walked(gen)
			);
		},
		linked() {
			if (walking) linkedWhileWalking = true;
			else reread();
		}
	};
}
