/**
 * Where a download in the dock stands, in words, for the menu items
 * that act on it. The row's bar is an indeterminate animation hidden
 * from assistive technology, and the store parses no percentage, so
 * this is what there is to say: queued, starting, or downloading (at
 * the speed the backend last reported), with the translated report a
 * running download carries. The dock puts it in the items'
 * description, read when an item is reached rather than announced as
 * it changes.
 */

import { m } from '$lib/paraglide/messages';
import { reportText } from './report-copy';
import { formatSpeed } from './speed';
import type { DownloadItem } from './store.svelte';

export interface RowState {
	/** The download's state; null for a finished or failed row, whose
	 *  own report says what there is to say. */
	readonly state: string | null;
	/** The report a running download carries; never the raw tool line,
	 *  which is not the user's language. */
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
		default:
			return { state: null, note: null };
	}
}
