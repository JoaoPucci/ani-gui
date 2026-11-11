// The app's link dependencies, built in one place for every surface.

import { afterEach, describe, expect, it, vi } from 'vitest';
import type { HistoryEntry } from '$lib/api';

const resolveKitsuMatchWithTrust = vi.hoisted(() =>
	vi.fn(async () => ({ match: null, trusted: false }))
);
vi.mock('./match', () => ({ resolveKitsuMatchWithTrust }));
const api = vi.hoisted(() => ({
	historyLink: vi.fn()
}));
vi.mock('$lib/api', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/api')>()),
	...api
}));

import { historyLinkGroup, historyList, kitsuSearch } from '$lib/api';
import { appLinkDeps, onHistoryLinked } from './link-deps';
import { resolveHistoryEntry } from './resolve';

describe('appLinkDeps', () => {
	it("hands the session the app's history, search and link routes", () => {
		const deps = appLinkDeps();
		expect(deps.history).toBe(historyList);
		expect(deps.search).toBe(kitsuSearch);
		expect(deps.group).toBe(historyLinkGroup);
	});

	it("matches a row as Continue Watching does, with Continue Watching's trust", async () => {
		const entry: HistoryEntry = {
			id: 'hianime:black-clover-949',
			title: 'Black Clover',
			ep_no: '1'
		};
		await appLinkDeps().resolve(entry);
		expect(resolveKitsuMatchWithTrust).toHaveBeenCalledWith(resolveHistoryEntry(entry, null));
	});

	it('hands the match the signal of the arrival that asks for it', async () => {
		const entry: HistoryEntry = {
			id: 'hianime:black-clover-949',
			title: 'Black Clover',
			ep_no: '1'
		};
		const arrival = new AbortController();
		await appLinkDeps().resolve(entry, { signal: arrival.signal });
		expect(resolveKitsuMatchWithTrust).toHaveBeenLastCalledWith(resolveHistoryEntry(entry, null), {
			signal: arrival.signal
		});
	});
});

describe("appLinkDeps' match", () => {
	it('rejects when the match could not be read, apart from a row matched to nothing', async () => {
		const entry: HistoryEntry = {
			id: 'hianime:black-clover-949',
			title: 'Black Clover',
			ep_no: '1'
		};
		resolveKitsuMatchWithTrust.mockResolvedValueOnce({
			match: null,
			trusted: false,
			unread: true
		} as never);
		await expect(appLinkDeps().resolve(entry)).rejects.toBeDefined();
		resolveKitsuMatchWithTrust.mockResolvedValueOnce({ match: null, trusted: false });
		await expect(appLinkDeps().resolve(entry)).resolves.toEqual({ match: null, trusted: false });
	});

	it('answers the match a guess found, though a read on the way failed', async () => {
		const entry: HistoryEntry = {
			id: 'hianime:black-clover-949',
			title: 'Black Clover',
			ep_no: '1'
		};
		const found = { id: '13209' } as never;
		resolveKitsuMatchWithTrust.mockResolvedValueOnce({
			match: found,
			trusted: false,
			unread: true
		} as never);
		await expect(appLinkDeps().resolve(entry)).resolves.toMatchObject({ match: found });
	});
});

// Every surface that reads the history hears of a link, made from any
// surface, whenever it answers: a link can answer after the page it was
// made on was left, and Home may be another instance by then.
describe('a link, once answered', () => {
	const unsubscribe: (() => void)[] = [];
	afterEach(() => {
		for (const off of unsubscribe.splice(0)) off();
		api.historyLink.mockReset();
	});

	it('is announced to every listener, after it answered', async () => {
		const heard: string[] = [];
		unsubscribe.push(onHistoryLinked(() => heard.push('one')));
		unsubscribe.push(onHistoryLinked(() => heard.push('two')));
		const outcome = { kitsu_id: '1', linked: [] };
		api.historyLink.mockImplementation(async () => {
			expect(heard).toEqual([]);
			return outcome;
		});
		expect(await appLinkDeps().link('card', ['card'], '1')).toBe(outcome);
		expect(heard).toEqual(['one', 'two']);
	});

	it('is announced when it failed too, since what it wrote is not known', async () => {
		const heard: number[] = [];
		unsubscribe.push(onHistoryLinked(() => heard.push(1)));
		api.historyLink.mockRejectedValue(new Error('down'));
		await expect(appLinkDeps().link('card', ['card'], '1')).rejects.toThrow('down');
		expect(heard).toEqual([1]);
	});

	it('answers as the write did, whatever a listener does', async () => {
		const heard: number[] = [];
		unsubscribe.push(
			onHistoryLinked(() => {
				throw new Error('listener');
			})
		);
		unsubscribe.push(onHistoryLinked(() => heard.push(1)));
		const outcome = { kitsu_id: '1', linked: [] };
		api.historyLink.mockResolvedValue(outcome);
		expect(await appLinkDeps().link('card', ['card'], '1')).toBe(outcome);
		expect(heard).toEqual([1]);
	});

	it('is not announced to a listener that left', async () => {
		const heard: number[] = [];
		onHistoryLinked(() => heard.push(1))();
		api.historyLink.mockResolvedValue({ kitsu_id: '1', linked: [] });
		await appLinkDeps().link('card', ['card'], '1');
		expect(heard).toEqual([]);
	});
});
