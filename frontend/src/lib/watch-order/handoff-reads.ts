/**
 * The reads behind Continue Watching's hand-offs, however often the
 * rail re-renders: each Kitsu id's franchise, asked once per page, and
 * whether a history row resumes the entry a card would move on to,
 * asked once per id until the history changes. A read that fails
 * settles as null (the row keeps its ordinary card, with no error).
 */
import type { FranchiseData } from './types';

export interface FranchiseReads {
	/** Asks for `id`'s franchise, unless it was asked for already. */
	want(id: string): void;
}

export function createFranchiseReads(
	read: (id: string) => Promise<FranchiseData>,
	settled: (id: string, franchise: FranchiseData | null) => void
): FranchiseReads {
	const asked = new Set<string>();
	return {
		want(id) {
			if (asked.has(id)) return;
			asked.add(id);
			read(id).then(
				(franchise) => settled(id, franchise),
				() => settled(id, null)
			);
		}
	};
}

export interface NamedReads extends FranchiseReads {
	/**
	 * Forgets what was asked, so the next want asks again, and drops
	 * the answers still out: the history changed, so a row may now
	 * resume an entry no row resumed before, or none may any more.
	 */
	forget(): void;
}

/**
 * Whether a history row resumes `id`, by the backend's resume lookup
 * (`read`, `historyByKitsuEach`): true or false, or null when the read
 * failed. Asked once per id until `forget`.
 */
export function createNamedReads(
	read: (ids: readonly string[]) => Promise<Readonly<Record<string, unknown>>>,
	settled: (id: string, named: boolean | null) => void
): NamedReads {
	let asked = new Set<string>();
	// Counts every forget: an answer is handed on only while none has
	// happened since it was asked.
	let generation = 0;
	return {
		want(id) {
			if (asked.has(id)) return;
			asked.add(id);
			const mine = generation;
			read([id]).then(
				(rows) => {
					if (mine === generation) settled(id, rows[id] != null);
				},
				() => {
					if (mine === generation) settled(id, null);
				}
			);
		},
		forget() {
			generation += 1;
			asked = new Set();
		}
	};
}
