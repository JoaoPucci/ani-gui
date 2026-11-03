import { afterEach, describe, expect, it, vi } from 'vitest';

const { forgetCachedEntry } = vi.hoisted(() => ({
	forgetCachedEntry: vi.fn(async () => undefined)
}));
vi.mock('./entry-api', () => ({ forgetCachedEntry }));

import { forgetCachedUnder } from './forget-cached';

afterEach(() => forgetCachedEntry.mockClear());

describe('forgetCachedUnder', () => {
	it('clears the row under the user the tracker was listed under', async () => {
		const forget = forgetCachedUnder({ anilist: { user: 'u-a' }, mal: { user: 'u-m' } });
		await forget('mal', '12');
		expect(forgetCachedEntry).toHaveBeenCalledWith('mal', '12', 'u-m');
	});

	it.each<Record<string, { user: string | null | undefined }>>([
		{ anilist: { user: null } },
		{ anilist: { user: undefined } },
		{}
	])('clears nothing for a tracker listed under no one: %j', async (listed) => {
		await forgetCachedUnder(listed)('anilist', '12');
		expect(forgetCachedEntry).not.toHaveBeenCalled();
	});
});
