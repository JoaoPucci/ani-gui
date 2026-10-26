import { describe, expect, it, vi } from 'vitest';

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
				{ getEntry, removeEntry },
				{ provider: 'mal', bearer: 'old', kitsuId: '12', held: nothingLeft }
			)
		).rejects.toBeInstanceOf(SupersededError);
		expect(getEntry).not.toHaveBeenCalled();
	});
});
