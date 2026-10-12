// Acceptance: the diagnostics page owns the one way to reach the hero
// framing switch. Its section has a single checkbox; a change is kept
// with the framing preferences, beside a framing and source already
// chosen, and the page still works where storage is refused.

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
import { HERO_PREFS_STORAGE_KEY } from '../../src/lib/hero/framing';
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
		(l.textContent ?? '').includes(m.diagnostics_banner_framing_toggle_label())
	);
	expect(label, 'the toggle should be labelled').toBeDefined();
	const input = label!.querySelector('input[type="checkbox"]');
	expect(input, 'the label should wrap a checkbox').not.toBeNull();
	return input as HTMLInputElement;
}

const stored = () => JSON.parse(localStorage.getItem(HERO_PREFS_STORAGE_KEY) ?? 'null');

describe('diagnostics — banner framing controls', () => {
	it('offers the toggle, off, in its own section', async () => {
		app = mount(DiagnosticsPage, { target });
		await settle();
		expect(target.textContent ?? '').toContain(m.diagnostics_section_banner_framing());
		expect(toggle().checked).toBe(false);
	});

	it('shows the stored state', async () => {
		localStorage.setItem(
			HERO_PREFS_STORAGE_KEY,
			JSON.stringify({ framing: 'focus', source: 'kitsu', controls: true })
		);
		app = mount(DiagnosticsPage, { target });
		await settle();
		expect(toggle().checked).toBe(true);
	});

	it('keeps a change beside the framing and source already chosen', async () => {
		localStorage.setItem(
			HERO_PREFS_STORAGE_KEY,
			JSON.stringify({ framing: 'focus', source: 'kitsu' })
		);
		app = mount(DiagnosticsPage, { target });
		await settle();
		const input = toggle();
		input.click();
		await settle();
		expect(input.checked).toBe(true);
		expect(stored()).toEqual({ framing: 'focus', source: 'kitsu', controls: true });
		input.click();
		await settle();
		expect(input.checked).toBe(false);
		expect(stored()).toEqual({ framing: 'focus', source: 'kitsu', controls: false });
	});

	it('takes a click where storage refuses the write, and keeps it for the session', async () => {
		// A full or sealed storage: reads answer, writes throw. (A storage
		// whose reads throw takes the whole app down through Paraglide's
		// locale lookup, which is nothing this page can answer for.)
		const before = JSON.stringify({ framing: 'top', source: 'kitsu' });
		const memory = new Map<string, string>([[HERO_PREFS_STORAGE_KEY, before]]);
		Object.defineProperty(globalThis, 'localStorage', {
			configurable: true,
			value: {
				getItem: (k: string) => memory.get(k) ?? null,
				setItem: () => {
					throw new DOMException('full', 'QuotaExceededError');
				}
			}
		});
		app = mount(DiagnosticsPage, { target });
		await settle();
		const input = toggle();
		expect(input.checked).toBe(false);
		expect(() => input.click()).not.toThrow();
		await settle();
		expect(input.checked).toBe(true);
		expect(memory.get(HERO_PREFS_STORAGE_KEY)).toBe(before);
	});
});
