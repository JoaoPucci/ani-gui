// The mapping from a parsed backend report to the locale's sentence,
// asserted through the same compiled messages the dock renders.
import { describe, expect, it } from 'vitest';

import { m } from '$lib/paraglide/messages';
import { progressTitle, reportText, terminalReportText } from './report-copy';
import type { DownloadItem } from './store.svelte';

function item(over: Partial<DownloadItem>): DownloadItem {
	return {
		id: 'dl-1',
		title: 'Frieren',
		episode: '7',
		mode: 'sub',
		quality: '1080',
		destDir: '/dl',
		status: 'active',
		progress: null,
		error: null,
		startedAt: 0,
		abort: null,
		unseen: false,
		rangeTotal: null,
		currentEp: null,
		progressStatus: null,
		...over
	};
}

describe('reportText', () => {
	it('interpolates the verbatim path into the claim sentences', () => {
		const path = '/dl/My Show Episode 1.mp4';
		expect(reportText({ key: 'abandoned_claim', path })).toBe(
			m.download_status_abandoned_claim({ path })
		);
		expect(reportText({ key: 'claim_pending', path })).toBe(
			m.download_status_claim_pending({ path })
		);
	});

	it('speaks each pathless report', () => {
		expect(reportText({ key: 'already_here', path: null })).toBe(m.download_status_already_here());
		expect(reportText({ key: 'repackage_retry', path: null })).toBe(
			m.download_status_repackage_retry()
		);
		expect(reportText({ key: 'retry_ffmpeg', path: null })).toBe(m.download_status_retry_ffmpeg());
	});
});

describe('progressTitle', () => {
	it('prefers the translated report over the raw line', () => {
		const it_ = item({
			progress: 'status.download.retry_ffmpeg',
			progressStatus: { key: 'retry_ffmpeg', path: null }
		});
		expect(progressTitle(it_)).toBe(m.download_status_retry_ffmpeg());
	});

	it('falls back to the raw tool line', () => {
		expect(progressTitle(item({ progress: '[download] 42%' }))).toBe('[download] 42%');
		expect(progressTitle(item({}))).toBe('');
	});
});

describe('terminalReportText', () => {
	it('renders the reports that explain an ended download', () => {
		const path = '/dl/My Show Episode 1.mp4';
		expect(terminalReportText(item({ progressStatus: { key: 'abandoned_claim', path } }))).toBe(
			m.download_status_abandoned_claim({ path })
		);
		expect(terminalReportText(item({ progressStatus: { key: 'already_here', path: null } }))).toBe(
			m.download_status_already_here()
		);
	});

	it('keeps the transient retries off terminal rows', () => {
		expect(
			terminalReportText(item({ progressStatus: { key: 'repackage_retry', path: null } }))
		).toBeNull();
		expect(terminalReportText(item({}))).toBeNull();
	});
});
