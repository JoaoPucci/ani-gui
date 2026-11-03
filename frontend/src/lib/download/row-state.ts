/**
 * Where a download in the dock stands, in words, for the menu items
 * that act on it. The row's bar is an indeterminate animation hidden
 * from assistive technology, and the store parses no percentage, so
 * this is what there is to say: queued, starting, downloading (at the
 * speed the backend last reported) with the translated report a running
 * download carries, downloaded, or failed with its reason. The dock
 * puts it in the items' description, which a screen reader reads when
 * an item is reached. It is no live region: a change is not announced
 * as one would announce it, though a screen reader may speak a large
 * change to the item that has focus.
 */

import { m } from '$lib/paraglide/messages';
import { reportText, terminalReportText } from './report-copy';
import { formatSpeed } from './speed';
import type { DownloadItem } from './store.svelte';

export interface RowState {
	/** The download's state. */
	readonly state: string;
	/** The report a running download carries, or why a failed one
	 *  failed; never the raw tool line, which is not the user's
	 *  language. */
	readonly note: string | null;
}

export function rowState(item: DownloadItem): RowState {
	switch (item.status) {
		case 'queued':
			return { state: m.download_dock_queued(), note: null };
		case 'pending':
			return { state: m.download_dock_state_starting(), note: null };
		case 'active':
			return {
				state:
					item.speed === null
						? m.download_dock_state_downloading()
						: m.download_dock_state_downloading_speed({ speed: formatSpeed(item.speed) }),
				note: item.progressStatus ? reportText(item.progressStatus) : null
			};
		case 'done':
			return { state: m.download_dock_state_done(), note: null };
		case 'error':
			// A failure over an abandoned or half-published file carries
			// a report naming it, which the row shows; the generic write
			// error would come first and point away from it.
			return {
				state: m.download_dock_state_failed(),
				note: terminalReportText(item) === null ? item.error : null
			};
	}
}
