/**
 * The detail page's resume row, read again once the page's own
 * availability lookup has answered. The page read the row when it
 * mounted; the lookup pays for the listing that can put the row's
 * numbering right — a stitched recap an earlier version numbered
 * "24.5" reads "12.5" after it — and the hero's resume should go on
 * from the episode the row reads as now.
 */

import type { HistoryEntry } from '$lib/api';
import { parseHistoryEpisode } from '$lib/play/episode-number';

/** `entry` as it reads now, with a position kept under the number it
 *  read as before moved to it; null when it reads as it did or cannot
 *  be read. */
export async function refreshedResume(
	entry: HistoryEntry,
	kitsuId: string,
	read: (id: string) => Promise<{ ep_no: string; former_ep_no?: string } | null>,
	move: (kitsuId: string, from: number, to: number) => void,
	warm?: (episode: number) => Promise<unknown>
): Promise<HistoryEntry | null> {
	void warm;
	let now: { ep_no: string; former_ep_no?: string } | null;
	try {
		now = await read(entry.id);
	} catch {
		return null;
	}
	if (!now || now.ep_no === entry.ep_no) return null;
	const to = parseHistoryEpisode(now.ep_no);
	const from = parseHistoryEpisode(now.former_ep_no);
	if (to !== null && from !== null && from !== to) move(kitsuId, from, to);
	return { ...entry, ep_no: now.ep_no };
}
