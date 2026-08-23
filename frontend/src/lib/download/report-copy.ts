/**
 * Rendered copy for the backend's own download reports.
 *
 * The backend sends stable `status.download.*` keys on the progress
 * stream — the store parses them into a {@link ProgressStatus} — and
 * this module is where a key becomes the locale's sentence. It sits
 * beside the store rather than inside the dock so the mapping is
 * unit-testable on its own: the component stays a thin adapter that
 * hands items in and renders strings out.
 */

import { m } from '$lib/paraglide/messages';
import { terminalReport, type DownloadItem, type ProgressStatus } from './store.svelte';

/** The translated sentence for one parsed report. */
export function reportText(s: ProgressStatus): string {
	switch (s.key) {
		case 'already_here':
			return m.download_status_already_here();
		case 'abandoned_claim':
			return m.download_status_abandoned_claim({ path: s.path ?? '' });
		case 'claim_pending':
			return m.download_status_claim_pending({ path: s.path ?? '' });
		case 'repackage_retry':
			return m.download_status_repackage_retry();
		case 'retry_ffmpeg':
			return m.download_status_retry_ffmpeg();
	}
}

/** Tooltip for an active row's progress bar: the translated report
 *  when the latest line is one, the raw tool line otherwise. */
export function progressTitle(item: DownloadItem): string {
	const s = item.progressStatus;
	return s ? reportText(s) : (item.progress ?? '');
}

/** What a finished or failed row still has to say, rendered visibly
 *  on the row. Only the reports that explain an ended download
 *  survive here — the store's classification — so a row that retried
 *  mid-flight and then failed for another reason does not claim the
 *  retry. */
export function terminalReportText(item: DownloadItem): string | null {
	const s = terminalReport(item.progressStatus);
	return s ? reportText(s) : null;
}
