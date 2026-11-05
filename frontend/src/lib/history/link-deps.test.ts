// The app's link dependencies, built in one place for every surface.

import { describe, expect, it, vi } from 'vitest';
import type { HistoryEntry } from '$lib/api';

const resolveKitsuMatchWithTrust = vi.hoisted(() =>
	vi.fn(async () => ({ match: null, trusted: false }))
);
vi.mock('./match', () => ({ resolveKitsuMatchWithTrust }));

import { historyLink, historyLinkGroup, historyList, historyUnlink, kitsuSearch } from '$lib/api';
import { appLinkDeps } from './link-deps';
import { resolveHistoryEntry } from './resolve';

describe('appLinkDeps', () => {
	it("hands the session the app's history, search and link routes", () => {
		const deps = appLinkDeps();
		expect(deps.history).toBe(historyList);
		expect(deps.search).toBe(kitsuSearch);
		expect(deps.link).toBe(historyLink);
		expect(deps.unlink).toBe(historyUnlink);
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
});
