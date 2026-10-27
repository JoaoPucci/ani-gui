import { describe, expect, it, vi } from 'vitest';
import { removeEntryAcrossTrackers, setEntryAcrossTrackers } from './set-entry';
import { AccountApiError } from './api';
import type { EntryView, ListEntry, ListStatus, Provider } from './types';

function fakeEntry(provider: Provider): ListEntry {
	return {
		provider,
		media_id: 21,
		mal_id: 21,
		status: 'watching',
		progress_episodes: 3,
		score_0_to_100: null,
		updated_at_epoch_s: 1,
		title: 'One Piece'
	};
}

const present: EntryView = { status: 'watching', progress: 3 };

describe('setEntryAcrossTrackers', () => {
	it('per provider: omits status where the row exists + status is unchanged, sends it where the row is missing', async () => {
		// anilist already has the title (keep its own status — send progress
		// only); mal lacks it (must be created with the editor's status).
		const getEntry = vi.fn(
			async (p: Provider): Promise<EntryView | null> => (p === 'anilist' ? present : null)
		);
		const setEntry = vi.fn(async (p: Provider) => fakeEntry(p));
		const out = await setEntryAcrossTrackers(
			{ connected: ['anilist', 'mal'], bearerFor: () => 'tok', getEntry, setEntry },
			'kitsu-12',
			{ status: 'watching', statusChanged: false, progress: 6 }
		);
		expect(out).toEqual({ written: 2, failed: 0 });
		expect(setEntry).toHaveBeenCalledWith('anilist', 'tok', { kitsu_id: 'kitsu-12', progress: 6 });
		expect(setEntry).toHaveBeenCalledWith('mal', 'tok', {
			kitsu_id: 'kitsu-12',
			status: 'watching',
			progress: 6
		});
	});

	it('sends status to every provider when the user changed it (a deliberate convergence)', async () => {
		const getEntry = vi.fn(async (): Promise<EntryView | null> => present);
		const setEntry = vi.fn(async (p: Provider) => fakeEntry(p));
		const out = await setEntryAcrossTrackers(
			{ connected: ['anilist', 'mal'], bearerFor: () => 'tok', getEntry, setEntry },
			'kitsu-12',
			{ status: 'paused', statusChanged: true, progress: 6 }
		);
		expect(out).toEqual({ written: 2, failed: 0 });
		for (const p of ['anilist', 'mal'] as Provider[]) {
			expect(setEntry).toHaveBeenCalledWith(p, 'tok', {
				kitsu_id: 'kitsu-12',
				status: 'paused',
				progress: 6
			});
		}
	});

	it('a provider with no bearer is a failure (unreachable), and is neither read nor written', async () => {
		const getEntry = vi.fn(async (): Promise<EntryView | null> => null);
		const setEntry = vi.fn(async (p: Provider) => fakeEntry(p));
		const out = await setEntryAcrossTrackers(
			{
				connected: ['anilist', 'mal'],
				bearerFor: (p) => (p === 'anilist' ? 'tok-a' : null),
				getEntry,
				setEntry
			},
			'kitsu-12',
			{ status: 'planning', statusChanged: false, progress: 0 }
		);
		expect(out).toEqual({ written: 1, failed: 1 });
		expect(getEntry).toHaveBeenCalledTimes(1);
		expect(setEntry).toHaveBeenCalledTimes(1);
	});

	it('an unmappable setEntry (null) is neither written nor failed; a throw is a failure', async () => {
		const getEntry = vi.fn(async (): Promise<EntryView | null> => null);
		const setEntry = vi.fn(async (p: Provider) => {
			if (p === 'anilist') throw new Error('network'); // failed
			if (p === 'mal') return null; // unmappable → neither
			return fakeEntry(p); // inhouse → written
		});
		const out = await setEntryAcrossTrackers(
			{ connected: ['anilist', 'mal', 'inhouse'], bearerFor: () => 'tok', getEntry, setEntry },
			'kitsu-12',
			{ status: 'planning', statusChanged: false, progress: 0 }
		);
		expect(out).toEqual({ written: 1, failed: 1 });
	});

	it('no-ops with no connected providers or empty kitsu id', async () => {
		const getEntry = vi.fn(async (): Promise<EntryView | null> => null);
		const setEntry = vi.fn(async (p: Provider) => fakeEntry(p));
		const save = { status: 'planning' as const, statusChanged: false, progress: 0 };
		expect(
			await setEntryAcrossTrackers(
				{ connected: [], bearerFor: () => 't', getEntry, setEntry },
				'k',
				save
			)
		).toEqual({ written: 0, failed: 0 });
		expect(
			await setEntryAcrossTrackers(
				{ connected: ['anilist'], bearerFor: () => 't', getEntry, setEntry },
				'',
				save
			)
		).toEqual({ written: 0, failed: 0 });
		expect(setEntry).not.toHaveBeenCalled();
	});
});

describe('removeEntryAcrossTrackers', () => {
	it('reports removed for present trackers only; an absent tracker is neither removed nor failed', async () => {
		// anilist has the row; mal does not. Only anilist is deleted; mal is
		// skipped (its no-op delete must not count toward success).
		const getEntry = vi.fn(
			async (p: Provider): Promise<EntryView | null> => (p === 'anilist' ? present : null)
		);
		const removeEntry = vi.fn(async () => undefined);
		const out = await removeEntryAcrossTrackers(
			{ connected: ['anilist', 'mal'], bearerFor: () => 'tok', getEntry, removeEntry },
			'kitsu-12'
		);
		expect(out).toEqual({ removed: 1, failed: 0 });
		expect(removeEntry).toHaveBeenCalledWith('anilist', 'tok', 'kitsu-12');
		expect(removeEntry).not.toHaveBeenCalledWith('mal', 'tok', 'kitsu-12');
	});

	it('a present tracker whose delete fails is reported as failed (partial removal)', async () => {
		// Both have the row; mal removes, anilist's delete throws. The result
		// must surface the failure so the editor keeps the entry visible
		// rather than claiming a clean removal.
		const getEntry = vi.fn(async (): Promise<EntryView | null> => present);
		const removeEntry = vi.fn(async (p: Provider) => {
			if (p === 'anilist') throw new Error('boom');
		});
		const out = await removeEntryAcrossTrackers(
			{ connected: ['anilist', 'mal'], bearerFor: () => 'tok', getEntry, removeEntry },
			'kitsu-12'
		);
		expect(out).toEqual({ removed: 1, failed: 1 });
	});

	it('no-ops with no connected providers', async () => {
		const getEntry = vi.fn(async (): Promise<EntryView | null> => present);
		const removeEntry = vi.fn(async () => undefined);
		expect(
			await removeEntryAcrossTrackers(
				{ connected: [], bearerFor: () => 't', getEntry, removeEntry },
				'k'
			)
		).toEqual({ removed: 0, failed: 0 });
		expect(removeEntry).not.toHaveBeenCalled();
	});
});

describe('rate-limit detection (429 surfaced distinctly)', () => {
	it('setEntryAcrossTrackers flags rateLimited when a provider call 429s', async () => {
		// A 429 still counts as failed (the write didn't land), but the flag
		// lets the editor show a "trackers are rate-limiting" toast instead of
		// the generic failure copy.
		const getEntry = vi.fn(async (): Promise<EntryView | null> => null);
		const setEntry = vi.fn(async () => {
			throw new AccountApiError(429, 'rate limited');
		});
		const out = await setEntryAcrossTrackers(
			{ connected: ['anilist'], bearerFor: () => 'tok', getEntry, setEntry },
			'kitsu-12',
			{ status: 'watching', statusChanged: false, progress: 5 }
		);
		expect(out.failed).toBe(1);
		expect(out.rateLimited).toBe(true);
	});

	it('setEntryAcrossTrackers does not flag rateLimited for a non-429 failure', async () => {
		const getEntry = vi.fn(async (): Promise<EntryView | null> => null);
		const setEntry = vi.fn(async () => {
			throw new AccountApiError(502, 'bad gateway');
		});
		const out = await setEntryAcrossTrackers(
			{ connected: ['anilist'], bearerFor: () => 'tok', getEntry, setEntry },
			'kitsu-12',
			{ status: 'watching', statusChanged: false, progress: 5 }
		);
		expect(out.failed).toBe(1);
		expect(out.rateLimited).toBeFalsy();
	});

	it('removeEntryAcrossTrackers flags rateLimited when the read 429s', async () => {
		const getEntry = vi.fn(async (): Promise<EntryView | null> => {
			throw new AccountApiError(429, 'rate limited');
		});
		const removeEntry = vi.fn(async () => undefined);
		const out = await removeEntryAcrossTrackers(
			{ connected: ['anilist'], bearerFor: () => 'tok', getEntry, removeEntry },
			'kitsu-12'
		);
		expect(out.failed).toBe(1);
		expect(out.rateLimited).toBe(true);
	});
});

// MyAnimeList has no rewatch status, and keeps "you finished this show" only in
// Completed: a Rewatching save never moves its row.
describe('setEntryAcrossTrackers — a rewatch on a tracker without one', () => {
	const completed: EntryView = { status: 'completed', progress: 26 };

	it('writes nothing to a MyAnimeList row that is Completed', async () => {
		const getEntry = vi.fn(async (): Promise<EntryView | null> => completed);
		const setEntry = vi.fn(async (p: Provider) => fakeEntry(p));
		const out = await setEntryAcrossTrackers(
			{ connected: ['anilist', 'mal'], bearerFor: () => 'tok', getEntry, setEntry },
			'kitsu-1',
			{ status: 'rewatching', statusChanged: true, progress: 3, total: 26 }
		);
		expect(setEntry).toHaveBeenCalledTimes(1);
		expect(setEntry).toHaveBeenCalledWith('anilist', 'tok', {
			kitsu_id: 'kitsu-1',
			status: 'rewatching',
			progress: 3
		});
		// MyAnimeList's row is as the save wants it — kept at Completed — so it
		// counts as written, not as a tracker the save failed to reach.
		expect(out).toEqual({ written: 2, failed: 0 });
	});

	it('is a clean save when MyAnimeList is kept and the other tracker cannot map the show', async () => {
		const getEntry = vi.fn(async (): Promise<EntryView | null> => completed);
		const setEntry = vi.fn(async (p: Provider) => (p === 'anilist' ? null : fakeEntry(p)));
		const out = await setEntryAcrossTrackers(
			{ connected: ['anilist', 'mal'], bearerFor: () => 'tok', getEntry, setEntry },
			'kitsu-1',
			{ status: 'rewatching', statusChanged: true, progress: 3, total: 26 }
		);
		expect(out).toEqual({ written: 1, failed: 0 });
	});

	it('never writes a Completed row below its count when the total is unknown', async () => {
		const getEntry = vi.fn(
			async (p: Provider): Promise<EntryView | null> =>
				p === 'anilist'
					? { status: 'rewatching', progress: 3 }
					: { status: 'completed', progress: 12 }
		);
		const setEntry = vi.fn(async (p: Provider) => fakeEntry(p));
		const out = await setEntryAcrossTrackers(
			{ connected: ['anilist', 'mal'], bearerFor: () => 'tok', getEntry, setEntry },
			'kitsu-1',
			{ status: 'rewatching', statusChanged: false, progress: 3, total: null }
		);
		expect(setEntry).not.toHaveBeenCalledWith('mal', expect.anything(), expect.anything());
		expect(setEntry).toHaveBeenCalledWith('anilist', 'tok', { kitsu_id: 'kitsu-1', progress: 3 });
		expect(out).toEqual({ written: 2, failed: 0 });
	});

	it('never lowers an AniList Completed row beside a MyAnimeList rewatch when the total is unknown', async () => {
		const getEntry = vi.fn(
			async (p: Provider): Promise<EntryView | null> =>
				p === 'mal' ? { status: 'rewatching', progress: 3 } : { status: 'completed', progress: 12 }
		);
		const setEntry = vi.fn(async (p: Provider) => fakeEntry(p));
		const out = await setEntryAcrossTrackers(
			{ connected: ['anilist', 'mal'], bearerFor: () => 'tok', getEntry, setEntry },
			'kitsu-1',
			{ status: 'rewatching', statusChanged: false, progress: 3, total: null }
		);
		expect(setEntry).not.toHaveBeenCalledWith('anilist', expect.anything(), expect.anything());
		expect(setEntry).toHaveBeenCalledWith('mal', 'tok', { kitsu_id: 'kitsu-1', progress: 3 });
		expect(out).toEqual({ written: 2, failed: 0 });
	});

	it('still sends a deliberate lower count on a Completed row whose total is unknown', async () => {
		const getEntry = vi.fn(
			async (): Promise<EntryView | null> => ({
				status: 'completed',
				progress: 12
			})
		);
		const setEntry = vi.fn(async (p: Provider) => fakeEntry(p));
		await setEntryAcrossTrackers(
			{ connected: ['anilist', 'mal'], bearerFor: () => 'tok', getEntry, setEntry },
			'kitsu-1',
			{ status: 'completed', statusChanged: false, progress: 10, total: null }
		);
		for (const p of ['anilist', 'mal'] as Provider[]) {
			expect(setEntry).toHaveBeenCalledWith(p, 'tok', { kitsu_id: 'kitsu-1', progress: 10 });
		}
	});

	it('sends a MyAnimeList row it reads as rewatching its progress alone', async () => {
		const getEntry = vi.fn(
			async (): Promise<EntryView | null> => ({
				status: 'rewatching',
				progress: 5
			})
		);
		const setEntry = vi.fn(async (p: Provider) => fakeEntry(p));
		await setEntryAcrossTrackers(
			{ connected: ['mal'], bearerFor: () => 'tok', getEntry, setEntry },
			'kitsu-1',
			{ status: 'rewatching', statusChanged: false, progress: 6, total: 26 }
		);
		expect(setEntry).toHaveBeenCalledWith('mal', 'tok', { kitsu_id: 'kitsu-1', progress: 6 });
	});

	it('sends MyAnimeList the progress alone on any other row', async () => {
		const getEntry = vi.fn(async (): Promise<EntryView | null> => present);
		const setEntry = vi.fn(async (p: Provider) => fakeEntry(p));
		await setEntryAcrossTrackers(
			{ connected: ['mal'], bearerFor: () => 'tok', getEntry, setEntry },
			'kitsu-1',
			{ status: 'rewatching', statusChanged: true, progress: 6, total: 26 }
		);
		expect(setEntry).toHaveBeenCalledWith('mal', 'tok', { kitsu_id: 'kitsu-1', progress: 6 });
	});

	it('creates no MyAnimeList row for a rewatch', async () => {
		const getEntry = vi.fn(
			async (p: Provider): Promise<EntryView | null> => (p === 'anilist' ? completed : null)
		);
		const setEntry = vi.fn(async (p: Provider) => fakeEntry(p));
		await setEntryAcrossTrackers(
			{ connected: ['anilist', 'mal'], bearerFor: () => 'tok', getEntry, setEntry },
			'kitsu-1',
			{ status: 'rewatching', statusChanged: true, progress: 3, total: 26 }
		);
		expect(setEntry).not.toHaveBeenCalledWith('mal', expect.anything(), expect.anything());
	});
});

// What the rewatch change must not move: every other status still reaches
// MyAnimeList as the user picked it.
describe('setEntryAcrossTrackers — every other status on MyAnimeList', () => {
	const cases: [ListStatus, number][] = [
		['watching', 6],
		['paused', 6],
		['dropped', 6],
		['planning', 0],
		['completed', 26]
	];
	for (const [status, progress] of cases) {
		it(`sends ${status} as picked`, async () => {
			const getEntry = vi.fn(async (): Promise<EntryView | null> => present);
			const setEntry = vi.fn(async (p: Provider) => fakeEntry(p));
			const out = await setEntryAcrossTrackers(
				{ connected: ['mal'], bearerFor: () => 'tok', getEntry, setEntry },
				'kitsu-1',
				{ status, statusChanged: true, progress: status === 'planning' ? 0 : 6, total: 26 }
			);
			expect(out).toEqual({ written: 1, failed: 0 });
			expect(setEntry).toHaveBeenCalledWith('mal', 'tok', {
				kitsu_id: 'kitsu-1',
				...(status === 'watching' ? {} : { status }),
				progress
			});
		});
	}
});
