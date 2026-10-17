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
 *  be read. `warm`, given when no listing stood behind the lookup,
 *  resolves a fractional row the read leaves as it was — the one kind
 *  a renumbering leaves under a number it no longer has — before it is
 *  read again. */
export async function refreshedResume(
	entry: HistoryEntry,
	kitsuId: string,
	read: (id: string) => Promise<{ ep_no: string; former_ep_no?: string } | null>,
	move: (kitsuId: string, from: number, to: number) => void,
	warm?: (episode: number) => Promise<unknown>
): Promise<HistoryEntry | null> {
	let now = await readRow(read, entry.id);
	const last = parseHistoryEpisode(now?.ep_no);
	if (warm && now && !now.former_ep_no && last !== null && !Number.isInteger(last)) {
		try {
			await warm(last);
			now = await readRow(read, entry.id);
		} catch {
			return null;
		}
	}
	if (!now) return null;
	const to = parseHistoryEpisode(now.ep_no);
	const from = parseHistoryEpisode(now.former_ep_no);
	const moved = to !== null && from !== null && from !== to;
	if (moved) move(kitsuId, from, to);
	// Given anew when the row names a former number, so Play is offered
	// again with the position where it is now.
	if (now.ep_no === entry.ep_no) return moved ? { ...entry } : null;
	return { ...entry, ep_no: now.ep_no };
}

async function readRow(
	read: (id: string) => Promise<{ ep_no: string; former_ep_no?: string } | null>,
	id: string
): Promise<{ ep_no: string; former_ep_no?: string } | null> {
	try {
		return await read(id);
	} catch {
		return null;
	}
}

/** Runs `refresh` on the resume row once both have happened — the row
 *  has arrived and the page's availability lookup has answered — in
 *  either order, and again for each later lookup that answers
 *  (`unresolved` as it goes out). `reset` starts a new visit. */
export function joinResumeRefresh(refresh: (entry: HistoryEntry) => void): {
	row: (entry: HistoryEntry | null) => void;
	resolved: () => void;
	unresolved: () => void;
	reset: () => void;
} {
	let row: HistoryEntry | null = null;
	let answered = false;
	let ran = false;
	const fire = () => {
		if (ran || !answered || !row) return;
		ran = true;
		refresh(row);
	};
	return {
		row: (entry) => {
			row = entry;
			fire();
		},
		resolved: () => {
			answered = true;
			fire();
		},
		unresolved: () => {
			answered = false;
			ran = false;
		},
		reset: () => {
			row = null;
			answered = false;
			ran = false;
		}
	};
}
