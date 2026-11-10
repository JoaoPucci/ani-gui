/**
 * The reads behind Continue Watching's hand-offs, each asked once per
 * page however often the rail re-renders: each Kitsu id's franchise,
 * and whether a history row resumes the entry a card would move on
 * to. A read that fails settles as null (the row keeps its ordinary
 * card, with no error).
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

/**
 * Whether a history row resumes `id`, by the backend's resume lookup
 * (`read`, `historyByKitsuEach`): true or false, or null when the read
 * failed. Asked once per id.
 */
export function createNamedReads(
	read: (ids: readonly string[]) => Promise<Readonly<Record<string, unknown>>>,
	settled: (id: string, named: boolean | null) => void
): FranchiseReads {
	void read;
	void settled;
	return { want() {} };
}
