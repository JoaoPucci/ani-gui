import { describe, expect, it, vi } from 'vitest';
import { makeStartResume, type ResumePlayArgs, type StartResumeDeps } from './start-resume';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';

function makeEntry(id: string, ep: string, title: string): HistoryEntry {
	return { id, ep_no: ep, title };
}

function makeMatch(id: string, episodeCount: number | null): KitsuAnimeRef {
	return {
		id,
		slug: `slug-${id}`,
		canonical_title: `Title ${id}`,
		titles: {},
		episode_count: episodeCount,
		subtype: 'TV',
		status: 'current',
		poster_image: null,
		start_date: null
	} as unknown as KitsuAnimeRef;
}

interface Harness {
	deps: StartResumeDeps;
	log: string[];
	busyLog: (string | null)[];
	counts: [string, number][];
	failures: [string, unknown, ResumePlayArgs][];
}

function makeHarness(overrides: Partial<StartResumeDeps> = {}): Harness {
	const log: string[] = [];
	const busyLog: (string | null)[] = [];
	const counts: [string, number][] = [];
	const failures: [string, unknown, ResumePlayArgs][] = [];
	const deps: StartResumeDeps = {
		isBusy: () => busyLog.at(-1) != null,
		onBusy: (id) => {
			busyLog.push(id);
			log.push(`busy:${id}`);
		},
		onProgress: () => {},
		onFailure: (title, e, args) => {
			failures.push([title, e, args]);
		},
		getSettings: vi.fn().mockImplementation(async () => {
			log.push('settings');
			return { mode: 'sub' as const, quality: 'best' };
		}),
		getPlayableCount: () => null,
		setPlayableCount: (id, c) => counts.push([id, c]),
		fetchInteractiveCount: vi.fn().mockImplementation(async () => {
			log.push('interactive-count');
			return { count: 12, approximate: false };
		}),
		resolvePlay: vi.fn().mockImplementation(async () => {
			log.push('resolve-play');
			return { session_id: 's1' };
		}),
		markWatched: vi.fn().mockResolvedValue(undefined),
		syncTrackers: vi.fn().mockResolvedValue(undefined),
		navigateToSession: vi.fn().mockImplementation(() => log.push('nav-session')),
		...overrides
	};
	return { deps, log, busyLog, counts, failures };
}

describe('makeStartResume (click orchestration)', () => {
	it('sequences busy -> settings -> interactive count -> play -> navigate', async () => {
		const h = makeHarness();
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '12', 'Show'), makeMatch('k1', 24), 24, false);

		expect(h.log).toEqual([
			'busy:k1',
			'settings',
			'interactive-count',
			'resolve-play',
			'nav-session'
		]);
		// Replay at the live cap: watched 12, live count 12 → episode 12.
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 12, mode: 'sub', quality: 'best' }),
			expect.any(Function)
		);
		expect(h.counts).toEqual([['h1', 12]]);
		// Busy stays set on success — navigation unmounts the page.
		expect(h.busyLog).toEqual(['k1']);
	});

	it('ignores a click while another resume is busy', async () => {
		const h = makeHarness({ isBusy: () => true });
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		expect(h.log).toEqual([]);
		expect(h.deps.resolvePlay).not.toHaveBeenCalled();
	});

	it('skips the interactive lookup when the probed cap is already in', async () => {
		const h = makeHarness({ getPlayableCount: () => 12 });
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 24), 24, false);
		expect(h.deps.fetchInteractiveCount).not.toHaveBeenCalled();
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 6 }),
			expect.any(Function)
		);
	});

	it('always resolves the episode: there is no player left behind to take back', async () => {
		// Leaving the play page unloads its video, so a click on the
		// episode just left resolves it like any other and the page
		// resumes from where it was left.
		const h = makeHarness();
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		expect(h.deps.resolvePlay).toHaveBeenCalledTimes(1);
		expect(h.deps.navigateToSession).toHaveBeenCalledTimes(1);
	});

	it('fires markWatched and tracker sync on success', async () => {
		const h = makeHarness();
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		expect(h.deps.markWatched).toHaveBeenCalledWith(expect.objectContaining({ episode: 6 }));
		expect(h.deps.syncTrackers).toHaveBeenCalledWith('k1', 6, 12, true);
	});

	it('a failed play resolution clears busy and reports the failure', async () => {
		const h = makeHarness({
			resolvePlay: vi.fn().mockRejectedValue(new Error('no sources'))
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		expect(h.busyLog).toEqual(['k1', null]);
		expect(h.failures).toEqual([
			[
				'Title k1',
				expect.any(Error),
				// What the failed play asked for, so the page can look up
				// the other audio of the same show and episode.
				expect.objectContaining({
					episode: 6,
					mode: 'sub',
					match: expect.objectContaining({ id: 'k1' })
				})
			]
		]);
		expect(h.deps.navigateToSession).not.toHaveBeenCalled();
	});

	it('stays busy while the failure surface words its message', async () => {
		// The page looks the other audio up before it shows the
		// failure. Clearing busy first left a window with neither the
		// loading state nor the failure on screen, in which a second
		// click could start a play the late message then covered.
		let release!: () => void;
		const worded = new Promise<void>((r) => (release = r));
		const h = makeHarness({
			resolvePlay: vi.fn().mockRejectedValue(new Error('no sources')),
			onFailure: () => worded
		});
		const start = makeStartResume(h.deps);
		const done = start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		await vi.waitFor(() => expect(h.deps.resolvePlay).toHaveBeenCalled());
		await Promise.resolve();
		await Promise.resolve();
		expect(h.busyLog, 'busy holds until the message is worded').toEqual(['k1']);
		expect(h.deps.isBusy()).toBe(true);
		release();
		await done;
		expect(h.busyLog).toEqual(['k1', null]);
	});

	it('a countless interactive lookup writes no cap and falls back to the Kitsu count', async () => {
		const h = makeHarness({
			fetchInteractiveCount: vi.fn().mockRejectedValue(new Error('down'))
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 24), 24, false);
		expect(h.counts).toEqual([]);
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 6 }),
			expect.any(Function)
		);
	});

	it('does nothing for a match without a canonical title', async () => {
		const match = makeMatch('k1', 12);
		(match as { canonical_title: string | null }).canonical_title = null;
		const h = makeHarness();
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), match, 12, true);
		expect(h.log).toEqual([]);
	});
});

describe('makeStartResume — progress and best-effort fan-out', () => {
	it('forwards resolution progress labels to the page', async () => {
		const labels: (string | null)[] = [];
		const h = makeHarness({
			onProgress: (l) => labels.push(l),
			resolvePlay: vi.fn().mockImplementation(async (_args, onProgress) => {
				onProgress('searching…');
				onProgress('provider ✓');
				return { session_id: 's1' };
			})
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		// null on entry (clearing the previous run), then each label.
		expect(labels).toEqual([null, 'searching…', 'provider ✓']);
	});

	it('still navigates when the watched-history write fails', async () => {
		// markWatched is best-effort: the episode is already resolved
		// and the user is owed the player regardless.
		const h = makeHarness({
			markWatched: vi.fn().mockRejectedValue(new Error('hsts locked'))
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		await Promise.resolve();
		expect(h.deps.navigateToSession).toHaveBeenCalled();
		expect(h.failures).toEqual([]);
	});

	it('still navigates when the tracker sync fails', async () => {
		const h = makeHarness({
			syncTrackers: vi.fn().mockRejectedValue(new Error('anilist 503'))
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);
		await Promise.resolve();
		expect(h.deps.navigateToSession).toHaveBeenCalled();
		expect(h.failures).toEqual([]);
	});

	it('opens a guessed match as a guess: no id recorded, no tracker write', async () => {
		const h = makeHarness({ isGuess: (id) => id === 'h1' });
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);

		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ guess: true }),
			expect.any(Function)
		);
		expect(h.deps.markWatched).toHaveBeenCalledWith(expect.objectContaining({ guess: true }));
		expect(h.deps.syncTrackers).not.toHaveBeenCalled();
		expect(h.deps.navigateToSession).toHaveBeenCalledWith(
			'k1',
			{ session_id: 's1' },
			6,
			true,
			'h1'
		);
	});

	it('opens a trusted match as the show itself', async () => {
		const h = makeHarness({ isGuess: () => false });
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, true);

		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ guess: false }),
			expect.any(Function)
		);
		expect(h.deps.syncTrackers).toHaveBeenCalledWith('k1', 6, 12, true);
		expect(h.deps.navigateToSession).toHaveBeenCalledWith(
			'k1',
			{ session_id: 's1' },
			6,
			false,
			'h1'
		);
	});
});

describe('makeStartResume — an episode left part-way', () => {
	it('goes back to the last watched episode when it was left part-way', async () => {
		const leftPartWay = vi.fn((kitsuId: string, ep: number) => kitsuId === 'k1' && ep === 5);
		const h = makeHarness({ getPlayableCount: () => 12, leftPartWay });
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, false);
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 5 }),
			expect.any(Function)
		);
		expect(h.deps.navigateToSession).toHaveBeenCalledWith(
			'k1',
			{ session_id: 's1' },
			5,
			false,
			'h1'
		);
	});

	it('goes on to the next episode when only an older one was left part-way', async () => {
		const h = makeHarness({
			getPlayableCount: () => 12,
			leftPartWay: (_k, ep) => ep === 4
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, false);
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 6 }),
			expect.any(Function)
		);
	});

	it("goes back to a series' last episode left part-way", async () => {
		const h = makeHarness({ getPlayableCount: () => 12, leftPartWay: () => true });
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '12', 'Show'), makeMatch('k1', 12), 12, true);
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 12 }),
			expect.any(Function)
		);
	});
});

describe('makeStartResume — the episode is marked started before its watch is recorded', () => {
	// The resolve records the watch, so a click that never reaches the
	// player — navigation cancelled, the app closed — must still leave
	// the episode marked started, or Continue reads it as finished.
	it('marks the episode it is about to play started before resolving it', async () => {
		const h = makeHarness({
			getPlayableCount: () => 12,
			markStarted: (k, ep) => h.log.push(`started:${k}:${ep}`)
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, false);
		expect(h.log).toEqual(['busy:k1', 'settings', 'started:k1:6', 'resolve-play', 'nav-session']);
	});

	it("marks it for the card's history row, which the session carries on", async () => {
		// The positions the session writes are the row's: a removed card
		// forgets them even after its guessed match is corrected.
		const h = makeHarness({
			getPlayableCount: () => 12,
			isGuess: () => true,
			markStarted: (k, ep, row) => h.log.push(`started:${k}:${ep}:${row}`)
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, false);
		expect(h.log).toContain('started:k1:6:h1');
		expect(h.deps.navigateToSession).toHaveBeenCalledWith(
			'k1',
			{ session_id: 's1' },
			6,
			true,
			'h1'
		);
	});
});

describe('makeStartResume — a fractional extra in history', () => {
	it('goes on from a recap to the whole episode after it', async () => {
		const h = makeHarness();
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '7.5', 'Show'), makeMatch('k1', 12), 12, true);
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 8 }),
			expect.anything()
		);
	});

	it('goes back to a recap left part-way', async () => {
		const asked: number[] = [];
		const h = makeHarness({
			leftPartWay: (_id, episode) => {
				asked.push(episode);
				return episode === 7.5;
			}
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '7.5', 'Show'), makeMatch('k1', 12), 12, true);
		expect(asked).toContain(7.5);
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 7.5 }),
			expect.anything()
		);
	});
});

describe('makeStartResume — a range-shaped row', () => {
	it('reads "1-12" by its first episode and plays 2', async () => {
		const h = makeHarness();
		await makeStartResume(h.deps)(makeEntry('h1', '1-12', 'Show'), makeMatch('k1', 12), 12, true);
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 2 }),
			expect.anything()
		);
	});
});

describe('makeStartResume (the row as it reads now)', () => {
	it('resumes from what the row reads as once the cap is settled, not what the card read', async () => {
		// The card read "24.5" when the page loaded — a stitched recap
		// numbered as an earlier version numbered it. The cap lookup
		// pays for the listing that puts it right; asked after it, the
		// row reads "12.5", and Continue goes on to 13, not to 25.
		const log: string[] = [];
		const h = makeHarness({
			fetchInteractiveCount: vi.fn().mockImplementation(async () => {
				log.push('interactive-count');
				return { count: 36, approximate: false };
			}),
			currentNumber: vi.fn().mockImplementation(async () => {
				log.push('current-number');
				return { ep_no: '12.5', former_ep_no: '24.5' };
			})
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '24.5', 'Show'), makeMatch('k1', 36), 36, false);
		expect(log).toEqual(['interactive-count', 'current-number']);
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 13 }),
			expect.any(Function)
		);
	});

	it('goes back to the row left part-way under the number it had before', async () => {
		// The recap was left part-way when it was "24.5": its position
		// moves to "12.5" before Continue asks whether it was.
		const kept = new Set(['k1:24.5']);
		const h = makeHarness({
			currentNumber: vi.fn().mockResolvedValue({ ep_no: '12.5', former_ep_no: '24.5' }),
			movePosition: vi.fn().mockImplementation((id: string, from: number, to: number) => {
				if (kept.delete(`${id}:${from}`)) kept.add(`${id}:${to}`);
			}),
			leftPartWay: (id, ep) => kept.has(`${id}:${ep}`)
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '24.5', 'Show'), makeMatch('k1', 36), 36, false);
		expect(h.deps.movePosition).toHaveBeenCalledWith('k1', 24.5, 12.5);
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 12.5 }),
			expect.any(Function)
		);
	});

	it('resumes from what the card read when the row cannot be asked', async () => {
		const h = makeHarness({
			currentNumber: vi.fn().mockRejectedValue(new Error('offline'))
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '5', 'Show'), makeMatch('k1', 12), 12, false);
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 6 }),
			expect.any(Function)
		);
	});

	it('opens the session on the episode it plays, and syncs that', async () => {
		const h = makeHarness({
			fetchInteractiveCount: vi.fn().mockResolvedValue({ count: 36, approximate: false }),
			resolvePlay: vi.fn().mockResolvedValue({ session_id: 's1', episode: '12.5' }),
			followPlayed: vi.fn().mockReturnValue(12.5)
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '24', 'Show'), makeMatch('k1', 36), 36, false);
		expect(h.deps.followPlayed).toHaveBeenCalledWith(
			'k1',
			expect.objectContaining({ episode: '12.5' }),
			25
		);
		expect(h.deps.navigateToSession).toHaveBeenCalledWith(
			'k1',
			expect.anything(),
			12.5,
			false,
			'h1'
		);
		expect(h.deps.syncTrackers).toHaveBeenCalledWith('k1', 12.5, 36, false);
	});
});

describe('makeStartResume (no listing behind the cap)', () => {
	it('warms the row it resumes from when nothing measured the cap, then reads it again', async () => {
		// The probe never answered and the cap lookup failed: no listing
		// has put the row right. Warming the row's own episode pays for
		// one, and the row then reads as it is.
		let warmed = false;
		const h = makeHarness({
			fetchInteractiveCount: vi.fn().mockRejectedValue(new Error('offline')),
			currentNumber: vi
				.fn()
				.mockImplementation(async () =>
					warmed ? { ep_no: '12.5', former_ep_no: '24.5' } : { ep_no: '24.5' }
				),
			warm: vi.fn().mockImplementation(async () => {
				warmed = true;
			})
		});
		const start = makeStartResume(h.deps);
		await start(makeEntry('h1', '24.5', 'Show'), makeMatch('k1', 36), 36, false);
		expect(h.deps.warm).toHaveBeenCalledWith(expect.objectContaining({ episode: 24.5 }));
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 13 }),
			expect.any(Function)
		);
	});

	it('warms nothing when a lookup measured the cap or the row is a whole episode', async () => {
		const measured = makeHarness({
			currentNumber: vi.fn().mockResolvedValue({ ep_no: '24.5' }),
			warm: vi.fn().mockResolvedValue(undefined)
		});
		await makeStartResume(measured.deps)(
			makeEntry('h1', '24.5', 'Show'),
			makeMatch('k1', 36),
			36,
			false
		);
		expect(measured.deps.warm).not.toHaveBeenCalled();
		const whole = makeHarness({
			fetchInteractiveCount: vi.fn().mockRejectedValue(new Error('offline')),
			warm: vi.fn().mockResolvedValue(undefined)
		});
		await makeStartResume(whole.deps)(makeEntry('h1', '5', 'Show'), makeMatch('k1', 36), 36, false);
		expect(whole.deps.warm).not.toHaveBeenCalled();
	});

	it('resumes from what it read when the warm fails', async () => {
		const h = makeHarness({
			fetchInteractiveCount: vi.fn().mockRejectedValue(new Error('offline')),
			currentNumber: vi.fn().mockResolvedValue({ ep_no: '24.5' }),
			warm: vi.fn().mockRejectedValue(new Error('offline'))
		});
		await makeStartResume(h.deps)(makeEntry('h1', '24.5', 'Show'), makeMatch('k1', 36), 36, false);
		expect(h.deps.resolvePlay).toHaveBeenCalledWith(
			expect.objectContaining({ episode: 25 }),
			expect.any(Function)
		);
	});
});

describe('makeStartResume (what the watch records)', () => {
	it('marks watched the episode the session plays', async () => {
		// The resolution the play left is kept under the episode it
		// played; a mark-watched for the request would find none.
		const h = makeHarness({
			fetchInteractiveCount: vi.fn().mockResolvedValue({ count: 36, approximate: false }),
			resolvePlay: vi.fn().mockResolvedValue({ session_id: 's1', episode: '12.5' }),
			followPlayed: vi.fn().mockReturnValue(12.5)
		});
		await makeStartResume(h.deps)(makeEntry('h1', '24', 'Show'), makeMatch('k1', 36), 36, false);
		expect(h.deps.markWatched).toHaveBeenCalledWith(expect.objectContaining({ episode: 12.5 }));
	});
});
