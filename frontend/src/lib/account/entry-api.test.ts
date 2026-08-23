/**
 * Tests for the detail-page list-entry editor endpoints. Backend routes
 * are mocked with `vi.spyOn(global, 'fetch')`, matching api.test.ts.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getEntry, removeEntry, setEntry } from './entry-api';
import { AccountApiError } from './api';

beforeEach(() => {
	(globalThis as { window?: unknown }).window = {
		aniGui: { apiBase: 'http://127.0.0.1:42337' }
	};
});

afterEach(() => {
	(globalThis as { window?: unknown }).window = undefined;
	vi.restoreAllMocks();
});

function mockFetchJson<T>(body: T, status = 200) {
	return vi.spyOn(global, 'fetch').mockResolvedValue(
		new Response(JSON.stringify(body), {
			status,
			headers: { 'content-type': 'application/json' }
		})
	);
}

describe('getEntry', () => {
	it('GETs /api/account/entry/:provider?kitsu_id= with bearer and returns the view', async () => {
		const view = { status: 'watching', progress: 5 };
		const spy = mockFetchJson(view);
		const r = await getEntry('anilist', 'tok', 'kitsu-12');
		expect(r).toEqual(view);
		const [url, init] = spy.mock.calls[0];
		expect(String(url)).toContain('/api/account/entry/anilist?kitsu_id=kitsu-12');
		expect((init as RequestInit | undefined)?.method ?? 'GET').toBe('GET');
		const headers = (init as RequestInit).headers as Record<string, string>;
		expect(headers.authorization).toBe('Bearer tok');
	});

	it('returns null when the show is not on the list', async () => {
		mockFetchJson(null);
		expect(await getEntry('mal', 'tok', 'kitsu-999')).toBeNull();
	});

	it('throws AccountApiError on non-2xx', async () => {
		mockFetchJson({}, 500);
		await expect(getEntry('anilist', 'tok', 'k')).rejects.toBeInstanceOf(AccountApiError);
	});
});

describe('setEntry', () => {
	it('POSTs kitsu_id + status + progress to /api/account/set/:provider', async () => {
		const entry = {
			provider: 'mal',
			media_id: 21,
			mal_id: 21,
			status: 'watching',
			progress_episodes: 3,
			score_0_to_100: null,
			updated_at_epoch_s: 1,
			title: 'One Piece'
		};
		const spy = mockFetchJson(entry);
		const r = await setEntry('mal', 'tok', {
			kitsu_id: 'kitsu-12',
			status: 'watching',
			progress: 3
		});
		expect(r).toEqual(entry);
		const [url, init] = spy.mock.calls[0];
		expect(String(url)).toContain('/api/account/set/mal');
		expect((init as RequestInit).method).toBe('POST');
		const body = String((init as RequestInit).body);
		expect(body).toContain('"kitsu_id":"kitsu-12"');
		expect(body).toContain('"status":"watching"');
		expect(body).toContain('"progress":3');
	});

	it('returns null when the show is unmappable', async () => {
		mockFetchJson(null);
		expect(await setEntry('anilist', 'tok', { kitsu_id: 'k', status: 'planning' })).toBeNull();
	});
});

describe('removeEntry', () => {
	it('DELETEs /api/account/entry/:provider?kitsu_id= with bearer', async () => {
		const spy = mockFetchJson({}, 200);
		await removeEntry('anilist', 'tok', 'kitsu-12');
		const [url, init] = spy.mock.calls[0];
		expect(String(url)).toContain('/api/account/entry/anilist?kitsu_id=kitsu-12');
		expect((init as RequestInit).method).toBe('DELETE');
		const headers = (init as RequestInit).headers as Record<string, string>;
		expect(headers.authorization).toBe('Bearer tok');
	});

	it('throws AccountApiError on non-2xx', async () => {
		mockFetchJson({}, 500);
		await expect(removeEntry('mal', 'tok', 'k')).rejects.toBeInstanceOf(AccountApiError);
	});
});
