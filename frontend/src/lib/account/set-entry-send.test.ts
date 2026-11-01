import { describe, expect, it, vi } from 'vitest';

import { AccountApiError } from './api';
import { sendRemoval, sendSave } from './set-entry-send';
import { SupersededError } from './save-when-allowed';

/** A held write with nothing left: its account changed, or newer writes
 *  took every field. */
const nothingLeft = { left: () => [] as const };

describe('a held write with nothing left', () => {
	it('does not read the tracker before finding it has nothing to save', async () => {
		const getEntry = vi.fn(async () => null);
		const setEntry = vi.fn(async () => null);
		await expect(
			sendSave(
				{ getEntry, setEntry },
				{ provider: 'mal', bearer: 'old', kitsuId: '12', held: nothingLeft },
				{ status: 'dropped', statusChanged: true, progress: 5 },
				['status', 'progress']
			)
		).rejects.toBeInstanceOf(SupersededError);
		expect(getEntry).not.toHaveBeenCalled();
		expect(setEntry).not.toHaveBeenCalled();
	});

	it('does not read the tracker before finding it has nothing to remove', async () => {
		const getEntry = vi.fn(async () => null);
		const removeEntry = vi.fn(async () => undefined);
		await expect(
			sendRemoval(
				{ getEntry, removeEntry, forgetCached: async () => undefined },
				{ provider: 'mal', bearer: 'old', kitsuId: '12', held: nothingLeft }
			)
		).rejects.toBeInstanceOf(SupersededError);
		expect(getEntry).not.toHaveBeenCalled();
	});
});

describe('a held removal a newer write overtakes while its read is out', () => {
	it('reads the row, then removes nothing', async () => {
		let left: readonly ('status' | 'progress')[] = ['status', 'progress'];
		const getEntry = vi.fn(async () => {
			// A newer progress write is issued while the read is out.
			left = ['status'];
			return { status: 'watching' as const, progress: 3 };
		});
		const removeEntry = vi.fn(async () => undefined);
		await expect(
			sendRemoval(
				{ getEntry, removeEntry, forgetCached: async () => undefined },
				{ provider: 'anilist', bearer: 't', kitsuId: '12', held: { left: () => left } }
			)
		).rejects.toBeInstanceOf(SupersededError);
		expect(getEntry).toHaveBeenCalledTimes(1);
		expect(removeEntry).not.toHaveBeenCalled();
	});
});

describe('reconciling an absent tracker entry', () => {
	const to = {
		provider: 'mal' as const,
		bearer: 't',
		kitsuId: '12',
		held: { left: () => ['status', 'progress'] as const }
	};

	it('clears the stale cached row locally and sends the tracker nothing', async () => {
		const removeEntry = vi.fn(async () => undefined);
		const forgetCached = vi.fn(async () => undefined);
		await expect(
			sendRemoval({ getEntry: async () => null, removeEntry, forgetCached }, to)
		).resolves.toBe('neither');
		expect(removeEntry).not.toHaveBeenCalled();
		expect(forgetCached).toHaveBeenCalledWith('mal', '12');
	});

	it('is not failed by a local clear that fails: the tracker is already clean', async () => {
		const forgetCached = vi.fn(async () => {
			throw new AccountApiError(500, '');
		});
		await expect(
			sendRemoval(
				{ getEntry: async () => null, removeEntry: async () => undefined, forgetCached },
				to
			)
		).resolves.toBe('neither');
	});

	it('keeps a present row whose delete fails failed, and clears nothing locally', async () => {
		const error = new AccountApiError(404, '{"kind":"upstream","status":404}');
		const forgetCached = vi.fn(async () => undefined);
		await expect(
			sendRemoval(
				{
					getEntry: async () => ({ status: 'planning' as const, progress: 0 }),
					removeEntry: async () => {
						throw error;
					},
					forgetCached
				},
				to
			)
		).rejects.toBe(error);
		expect(forgetCached).not.toHaveBeenCalled();
	});
});
