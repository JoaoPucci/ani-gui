/**
 * What a link session needs from the app, the same on every surface
 * that opens the picker: the show page, the player and Home
 * ($lib/history/link-offer.ts).
 *
 * Every link and undo is announced once it answered, failed or not,
 * to whatever reads the history and is on screen then: the notice
 * that holds an Undo outlives the page the link was made on, and the
 * Home on screen when it answers may not be the one it was made from.
 */

import { historyLink, historyLinkGroup, historyList, historyUnlink, kitsuSearch } from '$lib/api';
import type { LinkDeps } from './link-offer';
import { resolveKitsuMatchWithTrust } from './match';
import { resolveHistoryEntry } from './resolve';

export function appLinkDeps(): LinkDeps {
	return {
		history: historyList,
		resolve: (entry) => resolveKitsuMatchWithTrust(resolveHistoryEntry(entry, null)),
		search: kitsuSearch,
		link: (card, rows, kitsuId) => announced(historyLink(card, rows, kitsuId)),
		unlink: (kitsuId, rows) => announced(historyUnlink(kitsuId, rows)),
		group: historyLinkGroup
	};
}

const listeners = new Set<() => void>();

/** Hear of every link or undo once it answered; the way to stop. */
export function onHistoryLinked(listener: () => void): () => void {
	listeners.add(listener);
	return () => listeners.delete(listener);
}

/** `write`, announced once it answered, whatever it answered. */
function announced<T>(write: Promise<T>): Promise<T> {
	return write.finally(() => {
		for (const listener of [...listeners]) listener();
	});
}
