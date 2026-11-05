/**
 * What a link session needs from the app, the same on every surface
 * that opens the picker: the show page, the player and Home
 * ($lib/history/link-offer.ts).
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
		link: historyLink,
		unlink: historyUnlink,
		group: historyLinkGroup
	};
}
