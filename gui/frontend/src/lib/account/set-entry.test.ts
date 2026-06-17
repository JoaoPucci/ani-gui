import { describe, expect, it, vi } from 'vitest';
import { removeEntryAcrossTrackers, setEntryAcrossTrackers } from './set-entry';
import type { EntryView, ListEntry, Provider } from './types';

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
		const n = await setEntryAcrossTrackers(
			{ connected: ['anilist', 'mal'], bearerFor: () => 'tok', getEntry, setEntry },
			'kitsu-12',
			{ status: 'watching', seededStatus: 'watching', progress: 6 }
		);
		expect(n).toBe(2);
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
		const n = await setEntryAcrossTrackers(
			{ connected: ['anilist', 'mal'], bearerFor: () => 'tok', getEntry, setEntry },
			'kitsu-12',
			{ status: 'paused', seededStatus: 'watching', progress: 6 }
		);
		expect(n).toBe(2);
		for (const p of ['anilist', 'mal'] as Provider[]) {
			expect(setEntry).toHaveBeenCalledWith(p, 'tok', {
				kitsu_id: 'kitsu-12',
				status: 'paused',
				progress: 6
			});
		}
	});

	it('skips providers with no bearer (no read, no write)', async () => {
		const getEntry = vi.fn(async (): Promise<EntryView | null> => null);
		const setEntry = vi.fn(async (p: Provider) => fakeEntry(p));
		const n = await setEntryAcrossTrackers(
			{
				connected: ['anilist', 'mal'],
				bearerFor: (p) => (p === 'anilist' ? 'tok-a' : null),
				getEntry,
				setEntry
			},
			'kitsu-12',
			{ status: 'planning', seededStatus: 'planning', progress: 0 }
		);
		expect(n).toBe(1);
		expect(getEntry).toHaveBeenCalledTimes(1);
		expect(setEntry).toHaveBeenCalledTimes(1);
	});

	it('an unmappable setEntry (null) is not counted, and a throw does not block others', async () => {
		const getEntry = vi.fn(async (): Promise<EntryView | null> => null);
		const setEntry = vi.fn(async (p: Provider) => {
			if (p === 'anilist') throw new Error('network');
			if (p === 'mal') return null; // unmappable
			return fakeEntry(p);
		});
		const n = await setEntryAcrossTrackers(
			{ connected: ['anilist', 'mal', 'inhouse'], bearerFor: () => 'tok', getEntry, setEntry },
			'kitsu-12',
			{ status: 'planning', seededStatus: 'planning', progress: 0 }
		);
		expect(n).toBe(1); // only inhouse succeeded
	});

	it('no-ops with no connected providers or empty kitsu id', async () => {
		const getEntry = vi.fn(async (): Promise<EntryView | null> => null);
		const setEntry = vi.fn(async (p: Provider) => fakeEntry(p));
		const save = { status: 'planning' as const, seededStatus: 'planning' as const, progress: 0 };
		expect(
			await setEntryAcrossTrackers(
				{ connected: [], bearerFor: () => 't', getEntry, setEntry },
				'k',
				save
			)
		).toBe(0);
		expect(
			await setEntryAcrossTrackers(
				{ connected: ['anilist'], bearerFor: () => 't', getEntry, setEntry },
				'',
				save
			)
		).toBe(0);
		expect(setEntry).not.toHaveBeenCalled();
	});
});

describe('removeEntryAcrossTrackers', () => {
	it('counts only providers that actually had the entry', async () => {
		// anilist has the row; mal does not. Only anilist's delete counts —
		// the absent tracker mustn't inflate the success total.
		const getEntry = vi.fn(
			async (p: Provider): Promise<EntryView | null> => (p === 'anilist' ? present : null)
		);
		const removeEntry = vi.fn(async () => undefined);
		const n = await removeEntryAcrossTrackers(
			{ connected: ['anilist', 'mal'], bearerFor: () => 'tok', getEntry, removeEntry },
			'kitsu-12'
		);
		expect(n).toBe(1);
		expect(removeEntry).toHaveBeenCalledWith('anilist', 'tok', 'kitsu-12');
		expect(removeEntry).not.toHaveBeenCalledWith('mal', 'tok', 'kitsu-12');
	});

	it('a failed delete on the provider that had the row is not counted', async () => {
		const getEntry = vi.fn(async (): Promise<EntryView | null> => present);
		const removeEntry = vi.fn(async (p: Provider) => {
			if (p === 'anilist') throw new Error('boom');
		});
		const n = await removeEntryAcrossTrackers(
			{ connected: ['anilist', 'mal'], bearerFor: () => 'tok', getEntry, removeEntry },
			'kitsu-12'
		);
		expect(n).toBe(1); // mal removed; anilist failed → not counted
	});

	it('no-ops with no connected providers', async () => {
		const getEntry = vi.fn(async (): Promise<EntryView | null> => present);
		const removeEntry = vi.fn(async () => undefined);
		expect(
			await removeEntryAcrossTrackers(
				{ connected: [], bearerFor: () => 't', getEntry, removeEntry },
				'k'
			)
		).toBe(0);
		expect(removeEntry).not.toHaveBeenCalled();
	});
});
