// Acceptance: what the maintainer publishes in a signed flags file.
//
// The backend keeps the published notices, and the donation number,
// only from a flags file whose signature verifies, and serves them on
// /api/notices only while the notification center is on. So:
//
// 1. A verified notice: a toast says it, the bell counts it, the panel
//    lists it, its page shows its words as text and opens its link in
//    the user's browser. Read or dismissed, it stays so at the next
//    launch.
// 2. An unverified file: the backend serves nothing, and nothing shows.
// 3. With the center off, nothing is asked for and nothing shows.
// 4. The donation ask, once the number rises past the one the install
//    saw, outside the first fourteen days and with nothing playing:
//    one toast with Donate and Not now. Donate opens the About page's
//    Support section. Neither brings it back at the next launch.

import { describe, it, expect, vi, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';

import { page, setUrl } from './page-state.svelte';
import {
	QUIET,
	bell,
	bootLayout,
	installWindow,
	panel,
	server,
	settle,
	teardown,
	toastTexts
} from './notices-harness';
import { API_BASE } from './setup';

const goto = vi.fn(async (path: string) => void path);
vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));
vi.mock('$app/navigation', () => ({
	goto: (path: string) => goto(path),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn()
}));

import Layout from '../../src/routes/+layout.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { PUBLISHED_STORAGE_KEY } from '../../src/lib/notices/published-memory';
import { DONATION_STORAGE_KEY, GRACE_MS, serializeState } from '../../src/lib/notices/donation';

const published = {
	reads: 0,
	answer: { donation_ask: null, items: [] } as Record<string, unknown>
};

afterEach(async () => {
	await teardown();
	published.reads = 0;
	published.answer = { donation_ask: null, items: [] };
	window.localStorage.removeItem(PUBLISHED_STORAGE_KEY);
	window.localStorage.removeItem(DONATION_STORAGE_KEY);
	goto.mockClear();
});

const MAINT = {
	id: 'maint',
	title: 'Maintenance tonight',
	body: 'Back by 22:00.\n<b>not markup</b>',
	link: 'https://github.com/JoaoPucci/ani-gui',
	priority: 'warning',
	about_source: false
};

async function launch(flags: Record<string, boolean> | undefined, openExternal = vi.fn()) {
	__resetApiBaseForTests(API_BASE);
	setUrl('/');
	server.use(
		http.get(`${API_BASE}/api/notices`, () => {
			published.reads += 1;
			return HttpResponse.json(published.answer);
		}),
		...QUIET
	);
	installWindow(flags);
	(window as unknown as { aniGui: Record<string, unknown> }).aniGui.openExternal = openExternal;
	const Comp = Layout as unknown as Parameters<typeof bootLayout>[0];
	return bootLayout(Comp);
}

const rows = () => Array.from(panel()?.querySelectorAll<HTMLElement>('[data-row]') ?? []);
const buttonIn = (root: ParentNode, text: string) =>
	Array.from(root.querySelectorAll<HTMLButtonElement>('button')).find(
		(b) => b.textContent?.trim() === text
	);

describe('a notice from a verified flags file', () => {
	it('toasts, counts on the bell, lists, and opens its link in the browser', async () => {
		published.answer = { donation_ask: null, items: [MAINT] };
		const openExternal = vi.fn(async () => true);
		const root = await launch({ notification_center: true }, openExternal);
		expect(toastTexts()).toContain('Maintenance tonight');
		expect(bell(root)?.getAttribute('aria-label')).toBe('Notifications, 1 unread');
		bell(root)?.click();
		await settle();
		expect(rows()).toHaveLength(1);
		expect(rows()[0].textContent).toContain('Maintenance tonight');
		expect(rows()[0].textContent).toContain('From the developer');
		expect(rows()[0].querySelector('b')).toBeNull();
		buttonIn(rows()[0], 'Open link')?.click();
		await settle();
		expect(openExternal).toHaveBeenCalledWith('https://github.com/JoaoPucci/ani-gui');
	});

	it('stays read, or dismissed, at the next launch', async () => {
		published.answer = {
			donation_ask: null,
			items: [MAINT, { ...MAINT, id: 'other', title: 'Other' }]
		};
		let root = await launch({ notification_center: true });
		bell(root)?.click();
		await settle();
		const other = rows().find((r) => r.textContent?.includes('Other')) as HTMLElement;
		other.querySelector<HTMLButtonElement>('.nr-dismiss')?.click();
		await settle();
		buttonIn(panel() as HTMLElement, 'Mark all as read')?.click();
		await settle();
		await teardown();

		root = await launch({ notification_center: true });
		expect(toastTexts()).toEqual([]);
		expect(bell(root)?.getAttribute('aria-label')).toBe('Notifications');
		bell(root)?.click();
		await settle();
		expect(rows().map((r) => r.textContent?.includes('Maintenance tonight'))).toEqual([true]);
	});
});

describe('an unverified flags file', () => {
	it('has its notices dropped by the backend, so nothing shows', async () => {
		published.answer = { donation_ask: null, items: [] };
		const root = await launch({ notification_center: true });
		expect(published.reads).toBe(1);
		expect(toastTexts()).toEqual([]);
		bell(root)?.click();
		await settle();
		expect(rows()).toEqual([]);
	});
});

describe('with the center off', () => {
	it.each([
		['no flags', undefined],
		['the flag false', { notification_center: false }]
	])('asks for nothing published and shows nothing (%s)', async (_label, flags) => {
		published.answer = { donation_ask: 9, items: [MAINT] };
		window.localStorage.setItem(
			DONATION_STORAGE_KEY,
			serializeState({ v: 1, firstLaunchAt: 0, baseline: 1, acted: null, optOut: false })
		);
		const root = await launch(flags);
		expect(published.reads).toBe(0);
		expect(toastTexts()).toEqual([]);
		expect(bell(root)).toBeNull();
	});
});

describe('the donation ask', () => {
	function installedLongAgo(baseline: number) {
		window.localStorage.setItem(
			DONATION_STORAGE_KEY,
			serializeState({
				v: 1,
				firstLaunchAt: Date.now() - GRACE_MS - 86_400_000,
				baseline,
				acted: null,
				optOut: false
			})
		);
	}
	const ASK =
		"ani-gui is free and made in spare time. If it's useful to you, a donation helps keep it going.";

	it('offers Donate and Not now, and Donate opens the Support section', async () => {
		installedLongAgo(1);
		published.answer = { donation_ask: 2, items: [] };
		await launch({ notification_center: true });
		expect(toastTexts()).toEqual([ASK]);
		const toast = document.querySelector('.toast') as HTMLElement;
		const labels = Array.from(toast.querySelectorAll('button')).map((b) => b.textContent?.trim());
		expect(labels).toEqual(['Donate', 'Not now']);
		buttonIn(toast, 'Donate')?.click();
		await settle();
		expect(goto).toHaveBeenCalledWith('/about#support');
		expect(toastTexts()).toEqual([]);
	});

	it('goes with Not now and does not come back at the next launch', async () => {
		installedLongAgo(1);
		published.answer = { donation_ask: 2, items: [] };
		await launch({ notification_center: true });
		buttonIn(document.querySelector('.toast') as HTMLElement, 'Not now')?.click();
		await settle();
		expect(toastTexts()).toEqual([]);
		await teardown();
		published.answer = { donation_ask: 2, items: [] };
		await launch({ notification_center: true });
		expect(toastTexts()).toEqual([]);
	});

	it('records the number a new install sees, and does not ask for it', async () => {
		published.answer = { donation_ask: 5, items: [] };
		await launch({ notification_center: true });
		expect(toastTexts()).toEqual([]);
		expect(JSON.parse(window.localStorage.getItem(DONATION_STORAGE_KEY) as string).baseline).toBe(
			5
		);
	});
});
