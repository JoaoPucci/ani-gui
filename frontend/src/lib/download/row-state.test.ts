// What a download row in the dock says about where it stands, for the
// menu items that act on it: the visible bar is an animation, and a
// screen reader gets this instead.
import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { m } from '$lib/paraglide/messages';
import { reportText } from './report-copy';
import { rowState } from './row-state';
import { formatSpeed } from './speed';
import type { DownloadItem, ProgressStatusKey } from './store.svelte';

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
		speed: null,
		...over
	};
}

describe('rowState', () => {
	it('says a queued download is queued', () => {
		expect(rowState(item({ status: 'queued' }))).toEqual({
			state: m.download_dock_queued(),
			note: null
		});
	});

	it('says a pending download is starting', () => {
		expect(rowState(item({ status: 'pending' }))).toEqual({
			state: m.download_dock_state_starting(),
			note: null
		});
	});

	it('says a running download is downloading, at its speed once one is reported', () => {
		expect(rowState(item({ status: 'active' })).state).toBe(m.download_dock_state_downloading());
		expect(rowState(item({ status: 'active', speed: 2_500_000 })).state).toBe(
			m.download_dock_state_downloading_speed({ speed: formatSpeed(2_500_000) })
		);
	});

	it('adds the report a running download carries, and never the raw tool line', () => {
		const retry = item({ progressStatus: { key: 'retry_ffmpeg', path: null } });
		expect(rowState(retry).note).toBe(m.download_status_retry_ffmpeg());
		// The backend's yt-dlp template: format and bytes so far.
		const raw = item({ progress: 'ani-gui-progress 1080 123456' });
		expect(rowState(raw).note).toBeNull();
	});

	it('says a finished download is downloaded', () => {
		expect(rowState(item({ status: 'done' }))).toEqual({
			state: m.download_dock_state_done(),
			note: null
		});
	});

	it('says a failed download failed, and why', () => {
		const why = m.download_failure_tool_spawn();
		expect(rowState(item({ status: 'error', error: why }))).toEqual({
			state: m.download_dock_state_failed(),
			note: why
		});
		expect(rowState(item({ status: 'error', error: null }))).toEqual({
			state: m.download_dock_state_failed(),
			note: null
		});
	});

	it('answers a state for every download, and a running one’s note only from a report', () => {
		const keys: ProgressStatusKey[] = [
			'already_here',
			'abandoned_claim',
			'claim_pending',
			'repackage_retry',
			'retry_ffmpeg'
		];
		fc.assert(
			fc.property(
				fc.constantFrom('queued', 'pending', 'active', 'done') as fc.Arbitrary<
					DownloadItem['status']
				>,
				fc.option(fc.integer({ min: 0, max: 1e9 })),
				fc.option(fc.constantFrom(...keys)),
				fc.option(fc.string()),
				(status, speed, key, progress) => {
					const s = rowState(
						item({
							status,
							speed,
							progress,
							progressStatus: key === null ? null : { key, path: '/dl/x.mp4' }
						})
					);
					expect(typeof s.state).toBe('string');
					expect(s.state!.length).toBeGreaterThan(0);
					if (status !== 'active' || key === null) expect(s.note).toBeNull();
					else expect(s.note).toBe(reportText({ key, path: '/dl/x.mp4' }));
				}
			)
		);
	});
});
