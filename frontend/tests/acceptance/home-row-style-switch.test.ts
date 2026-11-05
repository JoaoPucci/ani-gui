// Acceptance: the home row style switch. Shown only while the
// diagnostics page's toggle is on; it names both styles in the user's
// language, presses the one in force, and applies a choice to every
// home row at once — no reload — keeping it for the next load.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mount, unmount } from 'svelte';

import { server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers, kitsuRef } from './home-handlers';
import { http, HttpResponse } from 'msw';
import { API_BASE } from './setup';

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

import HomePage from '../../src/routes/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { ROW_PREFS_STORAGE_KEY } from '../../src/lib/rows/prefs';
import { m } from '../../src/lib/paraglide/messages';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	localStorage.clear();
	server.use(
		...homeHandlers({}, [
			http.get(`${API_BASE}/api/kitsu/top-rated`, () =>
				HttpResponse.json([kitsuRef('7', 'Top Show', 12)])
			),
			http.post(`${API_BASE}/api/availability/batch`, () => HttpResponse.json({ cached: {} }))
		])
	);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	localStorage.clear();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}`);
}

const group = () =>
	target.querySelector(`[role="group"][aria-label="${m.rows_style_group_label()}"]`);
const choice = (label: string) =>
	Array.from(group()?.querySelectorAll('button') ?? []).find(
		(b) => b.textContent?.trim() === label
	) as HTMLButtonElement | undefined;
const surfaces = () =>
	new Set(
		Array.from(target.querySelectorAll('section.strip')).map((s) => s.getAttribute('data-surface'))
	);
/** The Top Rated row has rendered its card: the availability read
 *  behind it has answered, so nothing is still in flight at unmount. */
const settled = () => (target.textContent ?? '').includes('Top Show');
const stored = () => JSON.parse(localStorage.getItem(ROW_PREFS_STORAGE_KEY) ?? 'null');

describe('home row style switch', () => {
	it('is hidden unless the diagnostics toggle is on', async () => {
		app = mount(HomePage, { target });
		await until(settled, 'the Top Rated row');
		expect(group()).toBeNull();
		expect(surfaces()).toEqual(new Set(['plain']));
	});

	it('names both styles, the one in force pressed', async () => {
		localStorage.setItem(ROW_PREFS_STORAGE_KEY, JSON.stringify({ style: 'band', controls: true }));
		app = mount(HomePage, { target });
		await until(() => group() !== null && settled(), 'the switch and the Top Rated row');
		expect(choice(m.rows_style_inline())?.getAttribute('aria-pressed')).toBe('false');
		expect(choice(m.rows_style_band())?.getAttribute('aria-pressed')).toBe('true');
		expect(group()!.querySelectorAll('button')).toHaveLength(2);
	});

	it('applies a choice to every row at once and keeps it', async () => {
		const reload = vi.spyOn(window.location, 'reload').mockImplementation(() => {});
		localStorage.setItem(ROW_PREFS_STORAGE_KEY, JSON.stringify({ controls: true }));
		app = mount(HomePage, { target });
		await until(() => group() !== null && settled(), 'the switch and the Top Rated row');
		expect(surfaces()).toEqual(new Set(['plain']));

		choice(m.rows_style_band())!.click();
		await until(() => surfaces().has('band'), 'the rows to take the band');
		expect(surfaces()).toEqual(new Set(['band']));
		expect(stored()).toEqual({ style: 'band', controls: true });
		expect(choice(m.rows_style_band())?.getAttribute('aria-pressed')).toBe('true');

		choice(m.rows_style_inline())!.click();
		await until(() => surfaces().has('plain'), 'the rows to drop the band');
		expect(surfaces()).toEqual(new Set(['plain']));
		expect(stored()).toEqual({ style: 'inline', controls: true });
		expect(reload).not.toHaveBeenCalled();
		reload.mockRestore();
	});
});
