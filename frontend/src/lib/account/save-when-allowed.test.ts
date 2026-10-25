import { describe, expect, it, vi } from 'vitest';

import { AccountApiError } from './api';
import {
	busyWaitSecs,
	DEFAULT_WAIT_S,
	keepFields,
	MAX_SAVE_ATTEMPTS,
	MAX_WAIT_S,
	saveWhenAllowed,
	SupersededError
} from './save-when-allowed';

/** The backend's answer to a write it held back: 429 with the wait. */
const refused = (retryAfter: number | null) =>
	new AccountApiError(
		429,
		JSON.stringify({
			kind: 'rate_limited',
			key: 'error.network.rate_limited',
			retry_after_secs: retryAfter
		})
	);

describe('busyWaitSecs', () => {
	it('reads the wait a 429 advertises, rounded up', () => {
		expect(busyWaitSecs(refused(40))).toBe(40);
		expect(busyWaitSecs(refused(2.2))).toBe(3);
		expect(busyWaitSecs(refused(0))).toBe(0);
	});

	it('waits a minute on a 429 that names no wait, or whose body is not the envelope', () => {
		expect(busyWaitSecs(refused(null))).toBe(DEFAULT_WAIT_S);
		expect(busyWaitSecs(new AccountApiError(429, 'not json'))).toBe(DEFAULT_WAIT_S);
		expect(busyWaitSecs(new AccountApiError(429, '{"retry_after_secs":-4}'))).toBe(DEFAULT_WAIT_S);
		expect(busyWaitSecs(new AccountApiError(429, ''))).toBe(DEFAULT_WAIT_S);
	});

	it('is no wait for anything that is not a rate limit', () => {
		expect(busyWaitSecs(new AccountApiError(500, '{"retry_after_secs":5}'))).toBeNull();
		expect(busyWaitSecs(new Error('offline'))).toBeNull();
		expect(busyWaitSecs('429')).toBeNull();
		expect(busyWaitSecs(null)).toBeNull();
	});

	it('reads an error of the same shape from another module the same way', () => {
		// A test that mocks the api module hands the fan-out errors that are
		// not AccountApiError instances; the shape is what is read.
		const shaped = Object.assign(new Error('x'), { status: 429, detail: '{"retry_after_secs":7}' });
		expect(busyWaitSecs(shaped)).toBe(7);
	});
});

describe('saveWhenAllowed', () => {
	it('returns the first answer without waiting or saying anything', async () => {
		const notify = vi.fn();
		const wait = vi.fn(async () => {});
		expect(await saveWhenAllowed(async () => 'saved', { notify, wait })).toBe('saved');
		expect(notify).not.toHaveBeenCalled();
		expect(wait).not.toHaveBeenCalled();
	});

	it('says how long it waits, waits that long, and saves on the next attempt', async () => {
		const notify = vi.fn();
		const wait = vi.fn(async () => {});
		const attempt = vi
			.fn<() => Promise<string>>()
			.mockRejectedValueOnce(refused(40))
			.mockResolvedValueOnce('saved');
		expect(await saveWhenAllowed(attempt, { notify, wait })).toBe('saved');
		expect(notify).toHaveBeenCalledExactlyOnceWith(40);
		expect(wait).toHaveBeenCalledExactlyOnceWith(40_000);
		expect(attempt).toHaveBeenCalledTimes(2);
	});

	it('gives up after its attempts and rethrows the last refusal', async () => {
		const notify = vi.fn();
		const wait = vi.fn(async () => {});
		const last = refused(5);
		const attempt = vi.fn<() => Promise<string>>().mockRejectedValue(last);
		await expect(saveWhenAllowed(attempt, { notify, wait })).rejects.toBe(last);
		expect(attempt).toHaveBeenCalledTimes(MAX_SAVE_ATTEMPTS);
		expect(notify).toHaveBeenCalledTimes(MAX_SAVE_ATTEMPTS - 1);
	});

	it('does not wait out a failure that is not a rate limit', async () => {
		const notify = vi.fn();
		const wait = vi.fn(async () => {});
		const offline = new Error('offline');
		const attempt = vi.fn<() => Promise<string>>().mockRejectedValue(offline);
		await expect(saveWhenAllowed(attempt, { notify, wait, maxAttempts: 3 })).rejects.toBe(offline);
		expect(attempt).toHaveBeenCalledTimes(1);
		expect(notify).not.toHaveBeenCalled();
	});

	it('stops at the attempts the caller allows', async () => {
		const wait = vi.fn(async () => {});
		const attempt = vi.fn<() => Promise<string>>().mockRejectedValue(refused(1));
		await expect(
			saveWhenAllowed(attempt, { notify: () => {}, wait, maxAttempts: 2 })
		).rejects.toBeTruthy();
		expect(attempt).toHaveBeenCalledTimes(2);
	});
});

describe('the cap on one wait', () => {
	it('holds a write no longer than the cap, whatever the 429 asks', () => {
		expect(MAX_WAIT_S).toBe(120);
		expect(busyWaitSecs(refused(500))).toBe(MAX_WAIT_S);
		expect(busyWaitSecs(refused(120))).toBe(120);
		expect(busyWaitSecs(refused(119.5))).toBe(120);
	});
});

describe('a write no longer wanted after its wait', () => {
	it('is dropped as superseded, and not sent again', async () => {
		const notify = vi.fn();
		const attempt = vi.fn<() => Promise<string>>().mockRejectedValue(refused(3));
		await expect(
			saveWhenAllowed(attempt, { notify, wait: async () => {}, stillWanted: () => false })
		).rejects.toBeInstanceOf(SupersededError);
		expect(attempt).toHaveBeenCalledTimes(1);
	});

	it('goes again while it is still wanted', async () => {
		const attempt = vi
			.fn<() => Promise<string>>()
			.mockRejectedValueOnce(refused(3))
			.mockResolvedValueOnce('saved');
		expect(
			await saveWhenAllowed(attempt, {
				notify: () => {},
				wait: async () => {},
				stillWanted: () => true
			})
		).toBe('saved');
	});
});

describe('keepFields', () => {
	it('drops the declared fields a newer write covered, and nothing else', () => {
		const body = { kitsu_id: '12', status: 'paused', progress: 5 };
		expect(keepFields(body, ['status', 'progress'], ['status', 'progress'])).toEqual(body);
		expect(keepFields(body, ['status', 'progress'], ['status'])).toEqual({
			kitsu_id: '12',
			status: 'paused'
		});
		// A status the write sets without declaring it — the one a new row is
		// created with — rides along with a declared field that is left…
		expect(keepFields(body, ['progress'], ['progress'])).toEqual(body);
		// …and never goes on its own once none of the declared fields is.
		expect(keepFields(body, ['progress'], [])).toBeNull();
	});

	it('is nothing once neither a status nor a progress is left', () => {
		expect(keepFields({ kitsu_id: '12', progress: 5 }, ['progress'], [])).toBeNull();
		expect(keepFields({ status: 'paused', progress: 5 }, ['status', 'progress'], [])).toBeNull();
	});
});
