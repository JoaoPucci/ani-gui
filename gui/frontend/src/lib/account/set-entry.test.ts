import { describe, expect, it, vi } from 'vitest';
import { removeEntryAcrossTrackers, setEntryAcrossTrackers } from './set-entry';
import type { ListEntry, Provider } from './types';

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

describe('setEntryAcrossTrackers', () => {
	it('fans the edit out to every connected provider with its bearer + body', async () => {
		const setEntry = vi.fn(async (p: Provider) => fakeEntry(p));
		const n = await setEntryAcrossTrackers(
			{
				connected: ['anilist', 'mal'],
				bearerFor: (p) => (p === 'anilist' ? 'tok-a' : 'tok-m'),
				setEntry
			},
			'kitsu-12',
			{ status: 'watching', progress: 3 }
		);
		expect(n).toBe(2);
		expect(setEntry).toHaveBeenCalledWith('anilist', 'tok-a', {
			kitsu_id: 'kitsu-12',
			status: 'watching',
			progress: 3
		});
		expect(setEntry).toHaveBeenCalledWith('mal', 'tok-m', {
			kitsu_id: 'kitsu-12',
			status: 'watching',
			progress: 3
		});
	});

	it('omits absent fields and skips providers with no bearer', async () => {
		const setEntry = vi.fn(async (p: Provider) => fakeEntry(p));
		const n = await setEntryAcrossTrackers(
			{
				connected: ['anilist', 'mal'],
				bearerFor: (p) => (p === 'anilist' ? 'tok-a' : null),
				setEntry
			},
			'kitsu-12',
			{ progress: 5 }
		);
		expect(n).toBe(1);
		expect(setEntry).toHaveBeenCalledTimes(1);
		expect(setEntry).toHaveBeenCalledWith('anilist', 'tok-a', {
			kitsu_id: 'kitsu-12',
			progress: 5
		});
	});

	it('counts an unmappable provider (null) as not written, and a throw does not block others', async () => {
		const setEntry = vi.fn(async (p: Provider) => {
			if (p === 'anilist') throw new Error('network');
			if (p === 'mal') return null; // unmappable
			return fakeEntry(p);
		});
		const n = await setEntryAcrossTrackers(
			{ connected: ['anilist', 'mal', 'inhouse'], bearerFor: () => 'tok', setEntry },
			'kitsu-12',
			{ status: 'planning' }
		);
		expect(n).toBe(1); // only inhouse succeeded
		expect(setEntry).toHaveBeenCalledTimes(3);
	});

	it('no-ops with no connected providers or empty kitsu id', async () => {
		const setEntry = vi.fn(async (p: Provider) => fakeEntry(p));
		expect(
			await setEntryAcrossTrackers({ connected: [], bearerFor: () => 't', setEntry }, 'k', {})
		).toBe(0);
		expect(
			await setEntryAcrossTrackers(
				{ connected: ['anilist'], bearerFor: () => 't', setEntry },
				'',
				{}
			)
		).toBe(0);
		expect(setEntry).not.toHaveBeenCalled();
	});
});

describe('removeEntryAcrossTrackers', () => {
	it('removes from every connected provider and counts successes', async () => {
		const removeEntry = vi.fn(async () => undefined);
		const n = await removeEntryAcrossTrackers(
			{ connected: ['anilist', 'mal'], bearerFor: () => 'tok', removeEntry },
			'kitsu-12'
		);
		expect(n).toBe(2);
		expect(removeEntry).toHaveBeenCalledWith('anilist', 'tok', 'kitsu-12');
		expect(removeEntry).toHaveBeenCalledWith('mal', 'tok', 'kitsu-12');
	});

	it('best-effort: a throwing provider does not block the others', async () => {
		const removeEntry = vi.fn(async (p: Provider) => {
			if (p === 'anilist') throw new Error('boom');
		});
		const n = await removeEntryAcrossTrackers(
			{ connected: ['anilist', 'mal'], bearerFor: () => 'tok', removeEntry },
			'kitsu-12'
		);
		expect(n).toBe(1);
	});

	it('no-ops with no connected providers', async () => {
		const removeEntry = vi.fn(async () => undefined);
		expect(
			await removeEntryAcrossTrackers({ connected: [], bearerFor: () => 't', removeEntry }, 'k')
		).toBe(0);
		expect(removeEntry).not.toHaveBeenCalled();
	});
});
