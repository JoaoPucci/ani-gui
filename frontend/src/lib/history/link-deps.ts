/**
 * What a link session needs from the app, the same on every surface
 * that opens the picker: the show page, the player and Home
 * ($lib/history/link-offer.ts).
 *
 * Every link is announced once it answered, failed or not, to whatever
 * reads the history and is on screen then: a link can answer after
 * the page it was made on was left, and the Home or show page on
 * screen when it answers may not be the one it was made from.
 */

import {
	historyLink,
	historyLinkCandidates,
	historyLinkGroup,
	historyLinkVerdict,
	historyList,
	kitsuAnimeDetail
} from '$lib/api';
import type { LinkDeps } from './link-offer';
import { resolveKitsuMatchWithTrust } from './match';
import { readOpts } from './read-opts';
import { resolveHistoryEntry } from './resolve';

export function appLinkDeps(): LinkDeps {
	return {
		history: historyList,
		// No match where a read failed, or a row whose trust could not be
		// read, rejects: an offer made from it is not known, and is not
		// taken for one the row does not make.
		resolve: async (entry, opts) => {
			const read = await resolveKitsuMatchWithTrust(
				resolveHistoryEntry(entry, null),
				...readOpts(opts ?? {})
			);
			if (read.trustUnread || (read.unread && !read.match))
				throw new Error('the match could not be read');
			return read;
		},
		candidates: historyLinkCandidates,
		verdict: historyLinkVerdict,
		show: kitsuAnimeDetail,
		link: (card, rows, kitsuId) => announced(historyLink(card, rows, kitsuId)),
		group: historyLinkGroup
	};
}

const listeners = new Set<() => void>();

/** Hear of every link once it answered; the way to stop. */
export function onHistoryLinked(listener: () => void): () => void {
	listeners.add(listener);
	return () => listeners.delete(listener);
}

/** `write`, announced once it answered, whatever it answered. A
 *  listener's failure is its own: the write answers as it did, and the
 *  next listener still hears. */
function announced<T>(write: Promise<T>): Promise<T> {
	return write.finally(() => {
		for (const listener of [...listeners]) {
			try {
				listener();
			} catch {
				// Ignored: see above.
			}
		}
	});
}
