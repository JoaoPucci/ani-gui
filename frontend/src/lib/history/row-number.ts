/**
 * A Continue card's row, read again once its probe has answered. The
 * strip read every row when the page loaded; the probe pays for the
 * listing that can put a row's numbering right — a stitched recap an
 * earlier version numbered "24.5" reads "12.5" after it — and the card
 * should offer the episode after the one the row reads as now.
 */

import type { HistoryEntry } from '$lib/api';

/** The rows with `id`'s episode as it reads now, or null when it reads
 *  as it did, has gone, or cannot be read. */
export async function refreshedRows(
	id: string,
	rows: () => readonly HistoryEntry[] | null,
	read: (id: string) => Promise<{ ep_no: string } | null>
): Promise<HistoryEntry[] | null> {
	let now: { ep_no: string } | null;
	try {
		now = await read(id);
	} catch {
		return null;
	}
	const current = rows();
	const row = current?.find((r) => r.id === id);
	if (!now || !current || !row || row.ep_no === now.ep_no) return null;
	const epNo = now.ep_no;
	return current.map((r) => (r.id === id ? { ...r, ep_no: epNo } : r));
}
