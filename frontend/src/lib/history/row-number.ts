/**
 * A Continue card's row, read again once its probe has answered. The
 * strip read every row when the page loaded; the probe pays for the
 * listing that can put a row's numbering right — a stitched recap an
 * earlier version numbered "24.5" reads "12.5" after it — and the card
 * should offer the episode after the one the row reads as now.
 */

import type { HistoryEntry } from '$lib/api';
import { parseHistoryEpisode } from '$lib/play/episode-number';

/** The rows with `id`'s episode as it reads now, or null when it reads
 *  as it did, has gone, or cannot be read. A position kept under the
 *  number the row read as before its numbering was put right is moved
 *  (`move`) first, and the rows are given anew then even when the row
 *  reads as it did, so what reads the position reads it moved. */
export async function refreshedRows(
	id: string,
	rows: () => readonly HistoryEntry[] | null,
	read: (id: string) => Promise<{ ep_no: string; former_ep_no?: string } | null>,
	move?: (from: number, to: number) => void
): Promise<HistoryEntry[] | null> {
	let now: { ep_no: string; former_ep_no?: string } | null;
	try {
		now = await read(id);
	} catch {
		return null;
	}
	const current = rows();
	const row = current?.find((r) => r.id === id);
	if (!now || !current || !row) return null;
	const to = parseHistoryEpisode(now.ep_no);
	const from = parseHistoryEpisode(now.former_ep_no);
	const moved = !!move && to !== null && from !== null && from !== to;
	if (moved) move(from, to);
	if (row.ep_no === now.ep_no) return moved ? [...current] : null;
	const epNo = now.ep_no;
	return current.map((r) => (r.id === id ? { ...r, ep_no: epNo } : r));
}
