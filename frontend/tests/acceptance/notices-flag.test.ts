// Acceptance: the notification center sits behind its launch flag.
//
// The flag ships off. Off — no flag, or `notification_center: false`
// — the app is what it was before the center existed: no bell, no
// toast or row from it, no read of the source's health, nothing read
// from or written to its storage, and the update badge and download
// dock as they were. On, the bell is there. Turning it off and on
// again across launches is harmless: what an earlier launch stored is
// left alone while it is off and comes back when it is on.

import { describe, it, expect, vi, afterEach } from 'vitest';

import { page } from './page-state.svelte';
import {
	QUIET,
	bell,
	bootLayout,
	health,
	installWindow,
	server,
	teardown,
	toastTexts
} from './notices-harness';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));
vi.mock('$app/navigation', () => ({
	goto: vi.fn(async () => {}),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn()
}));

import Layout from '../../src/routes/+layout.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { downloadStore } from '../../src/lib/download/store.svelte';
import { updateStore } from '../../src/lib/update/store.svelte';
import { STORAGE_KEY } from '../../src/lib/notices/model';

afterEach(teardown);

const RELEASE = {
	tag: 'v9.0.0',
	name: 'v9.0.0',
	url: 'https://github.com/JoaoPucci/ani-gui/releases/tag/v9.0.0',
	publishedAt: '',
	body: ''
};

async function launch(flags: Record<string, boolean> | undefined) {
	__resetApiBaseForTests('http://127.0.0.1:31337');
	server.use(...QUIET);
	installWindow(flags);
	return bootLayout(Layout);
}

function finishADownload() {
	const id = downloadStore.add({
		title: 'Frieren',
		episode: '5',
		mode: 'sub',
		quality: 'best',
		destDir: '/home/u/Videos'
	});
	downloadStore.markDone(id, '/home/u/Videos');
}

describe('with the notification center off', () => {
	for (const [what, flags] of [
		['no launch flags', undefined],
		['the flag false', { notification_center: false }]
	] as const) {
		it(`shows no bell, raises nothing and reads nothing, with ${what}`, async () => {
			const root = await launch(flags);
			expect(bell(root)).toBeNull();
			updateStore.setAvailable(RELEASE);
			finishADownload();
			window.dispatchEvent(new Event('focus'));
			await new Promise((r) => setTimeout(r, 50));
			expect(root.querySelector('.update-badge'), 'the update badge as before').not.toBeNull();
			expect(root.querySelector('.dl-dock-trigger'), 'the download dock as before').not.toBeNull();
			expect(bell(root)).toBeNull();
			expect(toastTexts()).toEqual([]);
			expect(health.reads, 'no health read').toBe(0);
			expect(window.localStorage.getItem(STORAGE_KEY)).toBeNull();
		});
	}
});

describe('with the notification center on', () => {
	it('shows the bell and reads the source health', async () => {
		const root = await launch({ notification_center: true });
		expect(bell(root)).not.toBeNull();
		expect(bell(root)?.getAttribute('aria-label')).toBe('Notifications');
		expect(health.reads).toBeGreaterThan(0);
	});
});

describe('turning the center off and on again across launches', () => {
	it('leaves what it stored alone while off, and brings it back when on', async () => {
		await launch({ notification_center: true });
		finishADownload();
		await new Promise((r) => setTimeout(r, 50));
		const stored = window.localStorage.getItem(STORAGE_KEY);
		expect(stored).toContain('download-done:Frieren');
		await teardown();
		window.localStorage.setItem(STORAGE_KEY, stored as string);

		let root = await launch({ notification_center: false });
		expect(bell(root)).toBeNull();
		expect(window.localStorage.getItem(STORAGE_KEY)).toBe(stored);
		await teardown();
		window.localStorage.setItem(STORAGE_KEY, stored as string);

		root = await launch({ notification_center: true });
		expect(bell(root)?.getAttribute('aria-label')).toBe('Notifications, 1 unread');
	});
});
