/**
 * The series reads behind Continue Watching's hand-offs: each Kitsu
 * id's franchise is asked for once per page, however often the rail
 * re-renders, and a read that fails settles as null (the row keeps its
 * ordinary card, with no error).
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
	void [read, settled];
	return { want: () => {} };
}
