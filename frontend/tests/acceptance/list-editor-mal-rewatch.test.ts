// Acceptance: the list editor offers Rewatching with MyAnimeList connected.
//
// MyAnimeList has no rewatch status of its own; it holds a rewatch as a
// Watching row being rewatched, which is what the save sends it (unit-tested
// in set-entry). A rewatch is only ever set deliberately, here, so the
// editor offers it on every tracker, MyAnimeList alone included, and says
// nothing beside it.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));

vi.mock('$lib/account/set-entry', () => ({
	syncSetEntry: vi.fn(async () => ({ written: 1, failed: 0 })),
	syncRemoveEntry: vi.fn()
}));

import ListEntryEditor from '../../src/lib/components/ListEntryEditor.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { accountStore } from '../../src/lib/account/store.svelte';
import type { Provider } from '../../src/lib/account/types';

const ALL: Provider[] = ['anilist', 'mal'];

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

function connect(providers: Provider[]) {
	for (const provider of providers) {
		accountStore.setConnected(provider, {
			access_token: `${provider}-bearer`,
			refresh_token: null,
			expires_at_epoch_s: 4_000_000_000,
			user_id: 'user-1',
			username: 'tester',
			avatar_url: null
		});
	}
}

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	server.use();
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	for (const provider of ALL) accountStore.setDisconnected(provider);
	target.remove();
});

async function settle() {
	for (let i = 0; i < 20; i++) await new Promise((r) => setTimeout(r, 5));
}

const trigger = () => target.querySelector('button') as HTMLButtonElement;
const statusSelect = () => target.querySelector('select') as HTMLSelectElement | null;
const offered = () => Array.from(statusSelect()?.options ?? []).map((o) => o.value);
const note = () => target.querySelector('.le-note') !== null;

async function openOn(status: 'completed' | 'watching') {
	app = mount(ListEntryEditor, {
		target,
		props: { kitsuId: '1', total: 26, current: { status, progress: 26 } }
	});
	await settle();
	trigger().click();
	await settle();
}

describe('list editor with MyAnimeList connected', () => {
	it('offers Rewatching beside AniList, and says nothing once it is picked', async () => {
		connect(ALL);
		await openOn('completed');
		expect(offered()).toContain('rewatching');
		const select = statusSelect()!;
		select.value = 'rewatching';
		select.dispatchEvent(new Event('change', { bubbles: true }));
		await settle();
		expect(note()).toBe(false);
	});

	it('offers Rewatching with MyAnimeList alone', async () => {
		connect(['mal']);
		await openOn('completed');
		expect(statusSelect(), 'the status control should be offered').not.toBeNull();
		expect(offered()).toContain('rewatching');
		expect(offered()).toContain('completed');
	});
});
