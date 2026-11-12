// Acceptance: the diagnostics page owns the one way to reach the home
// row style switch, the way it owns the hero framing switch. Its
// section has a single checkbox; a change is kept with the row
// preferences, beside a style already chosen, and leaves the hero's
// preferences alone.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';

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

import DiagnosticsPage from '../../src/routes/diagnostics/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { HERO_PREFS_STORAGE_KEY } from '../../src/lib/hero/prefs';
import { ROW_PREFS_STORAGE_KEY } from '../../src/lib/rows/prefs';
import { m } from '../../src/lib/paraglide/messages';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

const g = globalThis as { localStorage?: unknown };
const storageDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	localStorage.clear();
	server.use(
		http.get(`${API_BASE}/api/app-info`, () =>
			HttpResponse.json({
				version: '0.15.1',
				history_path: '/home/u/.local/state/ani-gui/history',
				proxy_base_url: API_BASE,
				removed_legacy_paths: []
			})
		),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json([]))
	);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	if (storageDescriptor) Object.defineProperty(globalThis, 'localStorage', storageDescriptor);
	else delete g.localStorage;
	localStorage.clear();
});

async function settle() {
	for (let i = 0; i < 20; i++) await new Promise((r) => setTimeout(r, 5));
}

function toggle(): HTMLInputElement {
	const label = Array.from(target.querySelectorAll('label')).find((l) =>
		(l.textContent ?? '').includes(m.diagnostics_row_style_toggle_label())
	);
	expect(label, 'the toggle should be labelled').toBeDefined();
	const input = label!.querySelector('input[type="checkbox"]');
	expect(input, 'the label should wrap a checkbox').not.toBeNull();
	return input as HTMLInputElement;
}

const stored = () => JSON.parse(localStorage.getItem(ROW_PREFS_STORAGE_KEY) ?? 'null');

describe('diagnostics — home row style control', () => {
	it('offers the toggle, off, in its own section', async () => {
		app = mount(DiagnosticsPage, { target });
		await settle();
		expect(target.textContent ?? '').toContain(m.diagnostics_section_row_style());
		expect(toggle().checked).toBe(false);
	});

	it('shows the stored state', async () => {
		localStorage.setItem(ROW_PREFS_STORAGE_KEY, JSON.stringify({ style: 'band', controls: true }));
		app = mount(DiagnosticsPage, { target });
		await settle();
		expect(toggle().checked).toBe(true);
	});

	it('keeps a change beside the style already chosen, and leaves the hero’s alone', async () => {
		localStorage.setItem(ROW_PREFS_STORAGE_KEY, JSON.stringify({ style: 'band' }));
		const hero = JSON.stringify({ framing: 'focus', source: 'kitsu', controls: false });
		localStorage.setItem(HERO_PREFS_STORAGE_KEY, hero);
		app = mount(DiagnosticsPage, { target });
		await settle();
		const input = toggle();
		input.click();
		await settle();
		expect(input.checked).toBe(true);
		expect(stored()).toEqual({ style: 'band', controls: true });
		input.click();
		await settle();
		expect(input.checked).toBe(false);
		expect(stored()).toEqual({ style: 'band', controls: false });
		expect(localStorage.getItem(HERO_PREFS_STORAGE_KEY)).toBe(hero);
	});
});
