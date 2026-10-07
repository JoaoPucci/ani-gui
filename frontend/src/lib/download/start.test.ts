import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// Mocks must be declared before the module under test is imported.
// Vitest hoists vi.mock to the top of the file, so the relative
// imports inside `start.ts` resolve to these stubs at load time.

const apiMock = vi.hoisted(() => ({
	downloadStream: vi.fn()
}));
vi.mock('$lib/api', () => apiMock);

const storeMock = vi.hoisted(() => ({
	downloadStore: {
		add: vi.fn(),
		markActive: vi.fn(),
		setProgress: vi.fn(),
		markDone: vi.fn(),
		markError: vi.fn(),
		dismiss: vi.fn()
	}
}));
vi.mock('./store.svelte', () => storeMock);

const failureStoreMock = vi.hoisted(() => ({
	downloadFailureStore: {
		show: vi.fn(),
		dismiss: vi.fn()
	}
}));
vi.mock('./failure-store.svelte', () => failureStoreMock);

import { startDownload } from './start';
import { describeError, describePlayFailure } from '$lib/play/error-copy';

const baseArgs = {
	title: 'Demon Slayer',
	episode: '5',
	mode: 'sub',
	quality: '1080',
	destDir: '/tmp/dl'
};

describe('startDownload', () => {
	beforeEach(() => {
		storeMock.downloadStore.add.mockReset();
		storeMock.downloadStore.markActive.mockReset();
		storeMock.downloadStore.setProgress.mockReset();
		storeMock.downloadStore.markDone.mockReset();
		storeMock.downloadStore.markError.mockReset();
		storeMock.downloadStore.dismiss.mockReset();
		failureStoreMock.downloadFailureStore.show.mockReset();
		failureStoreMock.downloadFailureStore.dismiss.mockReset();
		apiMock.downloadStream.mockReset();
		// add() returns the new id; default to a known one so each
		// test can assert the lifecycle calls happened against it.
		storeMock.downloadStore.add.mockReturnValue('dl-1');
	});
	afterEach(() => {
		vi.useRealTimers();
	});

	it('adds a row, marks it active, and returns the id', async () => {
		apiMock.downloadStream.mockResolvedValueOnce({ dest_dir: '/tmp/dl' });
		const id = startDownload(baseArgs);
		expect(id).toBe('dl-1');
		expect(storeMock.downloadStore.add).toHaveBeenCalledWith({
			title: 'Demon Slayer',
			episode: '5',
			mode: 'sub',
			quality: '1080',
			destDir: '/tmp/dl'
		});
		expect(storeMock.downloadStore.markActive).toHaveBeenCalledTimes(1);
		// Second arg is the AbortController so the dock's cancel
		// button can hand it to .abort() later.
		const ctrlArg = storeMock.downloadStore.markActive.mock.calls[0][1];
		expect(ctrlArg).toBeInstanceOf(AbortController);
	});

	it("defaults quality to 'best' when caller omits it", () => {
		// The CLI accepts an empty quality but the dock label would
		// render an empty string. Keep the renderer's invariant
		// (quality is never empty) at this boundary.
		apiMock.downloadStream.mockReturnValueOnce(
			new Promise(() => {
				/* never resolves; we're only asserting the add() call */
			})
		);
		startDownload({ ...baseArgs, quality: undefined });
		expect(storeMock.downloadStore.add).toHaveBeenCalledWith(
			expect.objectContaining({ quality: 'best' })
		);
	});

	it('calls markDone with the dest_dir on success', async () => {
		apiMock.downloadStream.mockResolvedValueOnce({ dest_dir: '/var/anime' });
		startDownload(baseArgs);
		// Wait one microtask tick so the .then handler runs.
		await Promise.resolve();
		await Promise.resolve();
		expect(storeMock.downloadStore.markDone).toHaveBeenCalledWith('dl-1', '/var/anime');
		expect(storeMock.downloadStore.markError).not.toHaveBeenCalled();
	});

	it('forwards SSE progress lines to setProgress', () => {
		// downloadStream takes the on-progress callback as its second
		// arg. Capture it, fire two lines, and assert the store saw
		// the same payloads in order.
		let onLine: ((p: { line: string }) => void) | null = null;
		apiMock.downloadStream.mockImplementationOnce((_args, onp) => {
			onLine = onp as (p: { line: string }) => void;
			return new Promise(() => {
				/* never resolves; we're testing in-flight behaviour */
			});
		});
		startDownload(baseArgs);
		expect(onLine).not.toBeNull();
		onLine!({ line: '[download]  10.5% of 50.00MiB' });
		onLine!({ line: '[download] 100.0% of 50.00MiB' });
		expect(storeMock.downloadStore.setProgress).toHaveBeenNthCalledWith(
			1,
			'dl-1',
			'[download]  10.5% of 50.00MiB'
		);
		expect(storeMock.downloadStore.setProgress).toHaveBeenNthCalledWith(
			2,
			'dl-1',
			'[download] 100.0% of 50.00MiB'
		);
	});

	it('surfaces the rate-limit wait in the dock instead of a generic failure', async () => {
		// A throttled resolve rejects with the typed AniError envelope
		// { kind: 'rate_limited', retry_after_secs } — no `message`
		// property, so the generic branch would render "Download
		// failed" and discard the advertised wait. The dock row must
		// show the shared busy-source copy with the seconds instead.
		apiMock.downloadStream.mockRejectedValueOnce({
			kind: 'rate_limited',
			key: 'error.network.rate_limited',
			retry_after_secs: 9
		});
		startDownload(baseArgs);
		await Promise.resolve();
		await Promise.resolve();
		expect(storeMock.downloadStore.markError).toHaveBeenCalledWith(
			'dl-1',
			expect.stringMatching(/busy/i)
		);
		expect(storeMock.downloadStore.markError).toHaveBeenCalledWith(
			'dl-1',
			expect.stringMatching(/9/)
		);
		expect(failureStoreMock.downloadFailureStore.show).not.toHaveBeenCalled();
	});

	it('shows the episode verdict in the dock instead of a generic failure', async () => {
		// A download of an episode the provider does not carry rejects
		// with the typed envelope { kind: 'episode_unavailable', key }
		// — a unit variant, so no `message` and no `detail` — and the
		// generic branch would render "Download failed". The dock row
		// must carry the same episode copy the play page shows: the
		// show is there, this episode is not.
		apiMock.downloadStream.mockRejectedValueOnce({
			kind: 'episode_unavailable',
			key: 'error.play.episode_unavailable'
		});
		startDownload(baseArgs);
		await Promise.resolve();
		await Promise.resolve();
		expect(storeMock.downloadStore.markError).toHaveBeenCalledWith(
			'dl-1',
			expect.stringMatching(/this episode/i)
		);
		expect(storeMock.downloadStore.markError).not.toHaveBeenCalledWith('dl-1', 'Download failed');
		expect(failureStoreMock.downloadFailureStore.show).not.toHaveBeenCalled();
	});

	// The dock row's tooltip is user copy. A thrown Error's message
	// ("Download stream closed before completion.", a JSON parse
	// error), a bare string and a payload's `detail` are all internal
	// text, so every other failure gets describeError's sentence for
	// its kind instead.
	it('shows describeError copy for a thrown Error, never its message', async () => {
		apiMock.downloadStream.mockRejectedValueOnce(new Error('upstream 500'));
		startDownload(baseArgs);
		await Promise.resolve();
		await Promise.resolve();
		expect(storeMock.downloadStore.markError).toHaveBeenCalledWith(
			'dl-1',
			describeError(new Error('upstream 500'))
		);
		expect(storeMock.downloadStore.markError).not.toHaveBeenCalledWith('dl-1', 'upstream 500');
		expect(storeMock.downloadStore.markDone).not.toHaveBeenCalled();
	});

	it('shows describeError copy for a thrown string, never the string', async () => {
		apiMock.downloadStream.mockRejectedValueOnce('aborted by user');
		startDownload(baseArgs);
		await Promise.resolve();
		await Promise.resolve();
		expect(storeMock.downloadStore.markError).toHaveBeenCalledWith(
			'dl-1',
			describeError('aborted by user')
		);
	});

	it('names a typed failure by its kind and leaves its detail out', async () => {
		const failure = { kind: 'parse_failed', detail: 'missing field `dest_dir`' };
		apiMock.downloadStream.mockRejectedValueOnce(failure);
		startDownload(baseArgs);
		await Promise.resolve();
		await Promise.resolve();
		expect(storeMock.downloadStore.markError).toHaveBeenCalledWith('dl-1', describeError(failure));
		const shown = storeMock.downloadStore.markError.mock.calls[0][1] as string;
		expect(shown).not.toContain('dest_dir');
	});

	it('words a catalogue miss and a source outage the way the play page does', async () => {
		// A download resolves through the same provider as play, so these
		// two verdicts read the same on both surfaces.
		for (const failure of [{ kind: 'no_results' }, { kind: 'upstream', status: 503 }]) {
			storeMock.downloadStore.markError.mockReset();
			apiMock.downloadStream.mockRejectedValueOnce(failure);
			startDownload(baseArgs);
			await Promise.resolve();
			await Promise.resolve();
			expect(storeMock.downloadStore.markError).toHaveBeenCalledWith(
				'dl-1',
				describePlayFailure(failure)
			);
		}
	});

	it('words the failures a download has of its own for the download, not a service', async () => {
		// In a download these kinds mean something the general sentence
		// gets wrong: `timeout` includes the transfer's own one-hour
		// deadline, which is not a service being slow; `io` is the
		// destination folder refusing the file; `config` is no folder
		// being set; `scraper` is the download tool exiting non-zero.
		const cases: Array<[object, RegExp]> = [
			[{ kind: 'timeout', key: 'error.scraper.timeout' }, /download took too long/i],
			[{ kind: 'io', key: 'error.io.generic' }, /download folder/i],
			[{ kind: 'config', key: 'error.config.parse' }, /no download folder/i],
			[{ kind: 'scraper', key: 'error.scraper.parse_failed' }, /downloader/i]
		];
		for (const [failure, copy] of cases) {
			storeMock.downloadStore.markError.mockReset();
			apiMock.downloadStream.mockRejectedValueOnce(failure);
			startDownload(baseArgs);
			await Promise.resolve();
			await Promise.resolve();
			const shown = storeMock.downloadStore.markError.mock.calls[0]?.[1] as string;
			expect(shown, JSON.stringify(failure)).toMatch(copy);
			expect(shown, JSON.stringify(failure)).not.toBe(describeError(failure));
		}
	});

	it('falls back to localized generic copy for an unrecognised rejection', async () => {
		apiMock.downloadStream.mockRejectedValueOnce({ unexpected: true });
		startDownload(baseArgs);
		await Promise.resolve();
		await Promise.resolve();
		expect(storeMock.downloadStore.markError).toHaveBeenCalledWith(
			'dl-1',
			describeError({ unexpected: true })
		);
		expect(storeMock.downloadStore.markError).not.toHaveBeenCalledWith('dl-1', 'Download failed');
	});

	it('routes a typed ffmpeg_missing payload to the failure store and dismisses the dock row', async () => {
		// Backend SSE error event for a download started without
		// ffmpeg on PATH: the typed payload carries kind +
		// i18n key. The dock's bare "!" tooltip wouldn't help the
		// user; instead, hand the failure to the layout-level modal
		// (failure store) and clear the broken dock row so the user
		// sees one clear surface.
		apiMock.downloadStream.mockRejectedValueOnce({
			kind: 'ffmpeg_missing',
			key: 'error.download.ffmpeg_missing'
		});
		startDownload(baseArgs);
		await Promise.resolve();
		await Promise.resolve();
		expect(failureStoreMock.downloadFailureStore.show).toHaveBeenCalledWith({
			kind: 'ffmpeg_missing'
		});
		expect(storeMock.downloadStore.dismiss).toHaveBeenCalledWith('dl-1');
		// The broken-view path must NOT also fire — we don't want
		// the user seeing a half-closed dock row plus the modal.
		expect(storeMock.downloadStore.markError).not.toHaveBeenCalled();
	});
});
